use std::{cell::RefCell, rc::Rc};

use rand::{rngs::StdRng, Rng};

use crate::{
    contracts,
    oracle::Oracle,
    types::{AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ReplicaId},
    Replica,
};

#[derive(Debug)]
pub(crate) struct Simulator {
    config: SimulatorConfig,
    rng: StdRng,
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
        rng: StdRng,
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
            let i = self.rng.gen_range(0..self.replicas.len());
            let mut replica = self.replicas[i].as_ref().borrow_mut();
            replica.on_start_proposal(format!("value-{i}"));
            self.num_client_requests_sent += 1;
        }

        self.bus.tick();
    }
}

#[derive(Debug)]
pub(crate) struct SimMessageBus {
    replicas: RefCell<Vec<Rc<RefCell<Replica>>>>,
    queue: RefCell<Vec<PendingMessage>>,
    oracle: Oracle,
}

#[derive(Debug)]
enum PendingMessage {
    Prepare(ReplicaId, PrepareInput),
    PrepareResponse(ReplicaId, PrepareOutput),
    Accept(ReplicaId, AcceptInput),
    AcceptResponse(ReplicaId, AcceptOutput),
}

impl SimMessageBus {
    fn new(oracle: Oracle) -> Self {
        Self {
            replicas: RefCell::new(Vec::new()),
            queue: RefCell::new(Vec::new()),
            oracle,
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
        let message = {
            let mut queue = self.queue.borrow_mut();
            if queue.is_empty() {
                return;
            }
            queue.remove(0)
        };

        match message {
            PendingMessage::Prepare(to_replica_id, input) => {
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_prepare(input);
            }
            PendingMessage::PrepareResponse(to_replica_id, input) => {
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_prepare_response(input);
            }
            PendingMessage::Accept(to_replica_id, input) => {
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_accept(input);
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle.on_proposal_accepted(to_replica_id, &input);
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_accept_response(input);
            }
        }
    }
}

impl contracts::MessageBus for SimMessageBus {
    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput) {
        self.queue
            .borrow_mut()
            .push(PendingMessage::Prepare(to_replica_id, input));
    }

    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput) {
        self.queue
            .borrow_mut()
            .push(PendingMessage::PrepareResponse(to_replica_id, input));
    }

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput) {
        self.queue
            .borrow_mut()
            .push(PendingMessage::Accept(to_replica_id, input));
    }

    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput) {
        self.queue
            .borrow_mut()
            .push(PendingMessage::AcceptResponse(to_replica_id, input));
    }
}

#[cfg(test)]
mod tests {
    use rand::SeedableRng;

    use crate::{Config, Replica};

    use super::*;

    #[test]
    fn basic_simulation() {
        let seed: u64 = rand::thread_rng().gen();
        println!("seed={seed}");

        let rng = rand::rngs::StdRng::seed_from_u64(seed);

        let bus: Rc<SimMessageBus> = Rc::new(SimMessageBus::new(Oracle::new()));

        let servers = vec![1, 2, 3];

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
                num_client_requests: 1,
            },
            rng,
            replicas,
            bus,
        );

        for _ in 0..100 {
            sim.tick();
        }
    }
}
