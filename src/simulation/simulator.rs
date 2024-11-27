#![allow(clippy::unit_cmp)]

use core::iter::Iterator;
use std::{cell::RefCell, collections::HashSet, panic::UnwindSafe, path::PathBuf, rc::Rc};

use rand::{
    rngs::StdRng,
    seq::{IteratorRandom, SliceRandom},
};

use crate::{
    contracts::{self, MessageBus},
    file_storage::FileStorage,
    Replica, ReplicaId,
};

use super::{
    activity_log::ActivityLog,
    file_system::SimFileSystem,
    message_bus::{EventType, PendingMessage, SimMessageBus},
    oracle::Oracle,
};

#[derive(Debug)]
struct ActionSimulator {
    config: ActionSimulatorConfig,
    metrics: ActionSimulatorMetrics,
    action_set: Vec<Action>,
    rng: Rc<RefCell<StdRng>>,
    bus: Rc<SimMessageBus>,
    nodes: Vec<Node>,
    activity_log: Rc<RefCell<ActivityLog>>,
    healthy_replicas: HashSet<ReplicaId>,
    failed_replicas: HashSet<ReplicaId>,
}

impl UnwindSafe for ActionSimulator {}

#[derive(Debug)]
pub struct Node {
    pub replica: Replica,
    pub fs: Rc<SimFileSystem>,
}

#[derive(Debug)]
pub struct ActionSimulatorConfig {
    max_actions: u32,
    max_user_requests: u32,
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

#[derive(Debug, PartialEq, Clone, Copy)]
enum Action {
    SendUserRequest,
    CrashReplica,
    RestartReplica,
    DeliverMessage,
    DropMessage,
    DuplicateMessage,
}

impl Action {
    fn can_be_scheduled(
        &self,
        config: &ActionSimulatorConfig,
        metrics: &ActionSimulatorMetrics,
        num_messages_in_flight: usize,
    ) -> bool {
        match self {
            Action::SendUserRequest => metrics.num_user_requests_sent < config.max_user_requests,
            Action::CrashReplica => true,
            Action::RestartReplica => true,
            Action::DeliverMessage => num_messages_in_flight > 0,
            Action::DropMessage => num_messages_in_flight > 0,
            Action::DuplicateMessage => num_messages_in_flight > 0,
        }
    }
}

impl ActionSimulator {
    fn new(
        config: ActionSimulatorConfig,
        rng: Rc<RefCell<StdRng>>,
        nodes: Vec<Node>,
        bus: Rc<SimMessageBus>,
        activity_log: Rc<RefCell<ActivityLog>>,
    ) -> Self {
        Self {
            config,
            rng,
            bus,
            action_set: vec![
                Action::SendUserRequest,
                Action::CrashReplica,
                Action::RestartReplica,
                Action::DeliverMessage,
                Action::DropMessage,
                Action::DuplicateMessage,
            ],
            activity_log,
            healthy_replicas: HashSet::from_iter(nodes.iter().map(|node| node.replica.config.id)),
            failed_replicas: HashSet::new(),
            nodes,
            metrics: ActionSimulatorMetrics::new(),
        }
    }

    fn majority(&self) -> usize {
        self.nodes.len() / 2 + 1
    }

    fn next_action(&mut self) -> Action {
        let action = self
            .action_set
            .iter()
            .filter(|action| {
                action.can_be_scheduled(
                    &self.config,
                    &self.metrics,
                    self.bus.num_messages_in_flight(),
                )
            })
            .choose(unsafe { &mut *self.rng.as_ptr() })
            .cloned()
            .unwrap();

        match action {
            Action::SendUserRequest => {
                self.metrics.num_user_requests_sent += 1;
            }
            Action::CrashReplica => {
                self.metrics.num_replica_crashes += 1;
            }
            Action::RestartReplica => {
                self.metrics.num_replica_restarts += 1;
            }
            _ => {
                // noop.
            }
        }

        action
    }

    fn choose_healthy_replica(&mut self) -> &Replica {
        let replicas = self
            .nodes
            .iter()
            .filter(|node| self.healthy_replicas.contains(&node.replica.config.id));
        replicas
            .choose(unsafe { &mut *self.rng.as_ptr() })
            .map(|node| &node.replica)
            .unwrap()
    }

    fn choose_any_replica(&mut self) -> &Replica {
        self.nodes
            .choose(unsafe { &mut *self.rng.as_ptr() })
            .map(|node| &node.replica)
            .unwrap()
    }

    fn get_healthy_replica_index(&mut self, replica_id: ReplicaId) -> Option<usize> {
        self.nodes.iter().enumerate().find_map(|(i, node)| {
            if node.replica.config.id == replica_id
                && self.healthy_replicas.contains(&node.replica.config.id)
            {
                Some(i)
            } else {
                None
            }
        })
    }

    fn restart_node(&mut self, replica_id: ReplicaId) {
        for i in 0..self.nodes.len() {
            if self.nodes[i].replica.config.id == replica_id {
                let node = self.nodes.swap_remove(i);
                // Pretend the node restarted and the in the file system cache was lost.
                node.fs.restart();
                self.nodes.push(Node {
                    replica: Replica::new(
                        node.replica.config,
                        node.replica.bus,
                        Rc::new(
                            FileStorage::new(
                                Rc::clone(&node.fs) as Rc<dyn contracts::FileSystem>,
                                PathBuf::from("dir"),
                            )
                            .unwrap(),
                        ),
                    ),
                    ..node
                });
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
                    continue;
                    let replica_id = {
                        let replica = self.choose_any_replica();
                        replica.config.id
                    };
                    self.restart_node(replica_id);
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

        let node = &mut self.nodes[i];

        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Receive));

        match message {
            PendingMessage::StartProposal(_to_replica_id, value) => {
                node.replica.on_start_proposal(value);
            }
            PendingMessage::Prepare(_to_replica_id, input) => {
                node.replica.on_prepare(input);
            }
            PendingMessage::PrepareResponse(_to_replica_id, input) => {
                node.replica.on_prepare_response(input);
            }
            PendingMessage::Accept(_to_replica_id, input) => {
                node.replica.on_accept(input);
            }
            PendingMessage::AcceptResponse(_to_replica_id, input) => {
                node.replica.on_accept_response(input);
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use anyhow::Result;

    use rand::{Rng, SeedableRng};

    use crate::{
        file_storage::FileStorage,
        simulation::{
            file_system::SimFileSystem, in_memory_storage::InMemoryStorage,
            message_bus::SimMessageBus,
        },
        Config, Replica,
    };

    use super::*;

    #[test]
    fn action_simulation() -> Result<()> {
        let count = if std::env::var("SEED").is_ok() {
            1
        } else {
            std::env::var("MAX_THREADS")
                .map(|v| v.parse::<usize>().unwrap())
                .unwrap_or_else(|_| std::thread::available_parallelism().unwrap().get())
        };

        let max_iters = std::env::var("MAX_ITERS")
            .map(|v| v.parse::<u64>().unwrap())
            .unwrap_or_else(|_| 10_000);

        let max_actions = std::env::var("MAX_ACTIONS")
            .map(|v| v.parse::<u32>().unwrap())
            .unwrap_or_else(|_| 100);

        eprintln!("Spawning {count} threads");

        let handles = (0..count)
            .map(|thread_id| {
                std::thread::Builder::new()
                    .name(format!("Thread({thread_id})"))
                    .spawn(move || {
                        let seed: u64 = std::env::var("SEED")
                            .map(|v| v.parse::<u64>().unwrap())
                            .unwrap_or_else(|_| rand::thread_rng().gen());

                        eprintln!("SEED={seed}");

                        let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

                        for i in 0..max_iters {
                            if i % 1_000 == 0 {
                                eprintln!("Thread({thread_id}) Running simulation {i}");
                            }

                            let simulator_config = {
                                let mut rng = rng.borrow_mut();
                                ActionSimulatorConfig {
                                    max_actions,
                                    max_user_requests: rng.gen::<u32>() % 100 + 1,
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

                            let nodes: Vec<_> = servers
                                .iter()
                                .map(|id| {
                                    let fs = Rc::new(SimFileSystem::new());
                                    Node {
                                        replica: Replica::new(
                                            Config {
                                                id: *id,
                                                replicas: servers.clone(),
                                            },
                                            Rc::clone(&bus) as Rc<dyn contracts::MessageBus>,
                                            Rc::new(
                                                FileStorage::new(
                                                    Rc::clone(&fs) as Rc<dyn contracts::FileSystem>,
                                                    PathBuf::from("dir"),
                                                )
                                                .unwrap(),
                                            ),
                                        ),
                                        fs,
                                    }
                                })
                                .collect();

                            let mut sim = ActionSimulator::new(
                                simulator_config,
                                Rc::clone(&rng),
                                nodes,
                                Rc::clone(&bus),
                                Rc::clone(&activity_log),
                            );

                            let result = std::panic::catch_unwind(move || {
                                sim.run();
                                assert!(sim.bus.is_empty());
                            });
                            if result.is_err() {
                                activity_log.borrow_mut().print_events();
                                eprintln!("SEED={seed}");
                                std::process::exit(1);
                            }
                        }
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        for handle in handles {
            handle.join().unwrap();
        }

        Ok(())
    }
}
