use std::{cell::RefCell, rc::Rc};

use rand::{rngs::StdRng, Rng, SeedableRng};

type ReplicaId = u32;

#[derive(Debug)]
struct Replica {
    min_proposal_number: u64,
    accepted_proposal_number: Option<u64>,
    accepted_value: Option<String>,

    config: Config,
    bus: Rc<dyn MessageBus>,
}

#[derive(Debug)]
struct Config {
    id: ReplicaId,
    replicas: Vec<ReplicaId>,
}

#[derive(Debug)]
struct PrepareInput {
    from_replica_id: ReplicaId,
    request_id: u64,
    proposal_number: u64,
}

#[derive(Debug)]
struct PrepareOutput {
    from_replica_id: ReplicaId,
    request_id: u64,
    accepted_proposal_number: Option<u64>,
    accepted_value: Option<String>,
}

#[derive(Debug)]
struct AcceptInput {
    from_replica_id: ReplicaId,
    request_id: u64,
    proposal_number: u64,
    value: String,
}

#[derive(Debug)]
struct AcceptOutput {
    from_replica_id: ReplicaId,
    request_id: u64,
    min_proposal_number: u64,
}

trait MessageBus: std::fmt::Debug {
    fn send_prepare(&self, to_replica_id: u32, input: PrepareInput);
    fn send_prepare_response(&self, to_replica_id: u32, input: PrepareOutput);

    fn send_accept(&self, to_replica_id: u32, input: AcceptInput);
    fn send_accept_response(&self, to_replica_id: u32, input: AcceptOutput);
}

impl Replica {
    fn new(config: Config, bus: Rc<dyn MessageBus>) -> Self {
        Self {
            min_proposal_number: 0,
            accepted_proposal_number: None,
            accepted_value: None,
            config,
            bus,
        }
    }

    fn on_start_proposal(&mut self, value: String) {
        dbg!(&value);
    }

    fn on_prepare(&mut self, input: PrepareInput) {
        if input.proposal_number > self.min_proposal_number {
            self.min_proposal_number = input.proposal_number;
            self.bus.send_prepare_response(
                input.from_replica_id,
                PrepareOutput {
                    from_replica_id: self.config.id,
                    request_id: input.request_id,
                    accepted_proposal_number: self.accepted_proposal_number,
                    accepted_value: self.accepted_value.clone(),
                },
            );
        }
    }

    fn on_prepare_response(&mut self, input: PrepareOutput) {}

    fn on_accept(&mut self, input: AcceptInput) {
        if input.proposal_number >= self.min_proposal_number {
            self.accepted_proposal_number = Some(input.proposal_number);
            self.accepted_value = Some(input.value);
            self.bus.send_accept_response(
                input.from_replica_id,
                AcceptOutput {
                    from_replica_id: self.config.id,
                    request_id: input.request_id,
                    min_proposal_number: self.min_proposal_number,
                },
            );
        }
    }

    fn on_accept_response(&mut self, input: AcceptOutput) {}
}

#[derive(Debug)]
struct Simulator {
    config: SimulatorConfig,
    rng: StdRng,
    replicas: Vec<Rc<RefCell<Replica>>>,
    num_client_requests_sent: u32,
}

#[derive(Debug)]
struct SimulatorConfig {
    num_client_requests: u32,
}

impl Simulator {
    fn new(config: SimulatorConfig, rng: StdRng, replicas: Vec<Rc<RefCell<Replica>>>) -> Self {
        Self {
            config,
            rng,
            replicas,
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
    }
}

#[derive(Debug)]
struct SimMessageBus {
    replicas: RefCell<Vec<Rc<RefCell<Replica>>>>,
}

impl SimMessageBus {
    fn new() -> Self {
        Self {
            replicas: RefCell::new(Vec::new()),
        }
    }
}

impl SimMessageBus {
    fn set_replicas(&self, new_replicas: Vec<Rc<RefCell<Replica>>>) {
        let mut replicas = self.replicas.borrow_mut();
        replicas.clear();
        replicas.extend(new_replicas);
    }
}

impl MessageBus for SimMessageBus {
    fn send_prepare(&self, to_replica_id: u32, input: PrepareInput) {
        todo!()
    }

    fn send_prepare_response(&self, to_replica_id: u32, input: PrepareOutput) {
        todo!()
    }

    fn send_accept(&self, to_replica_id: u32, input: AcceptInput) {
        todo!()
    }

    fn send_accept_response(&self, to_replica_id: u32, input: AcceptOutput) {
        todo!()
    }
}

fn main() {
    let seed: u64 = rand::thread_rng().gen();
    println!("seed={seed}");

    let rng = rand::rngs::StdRng::seed_from_u64(seed);

    let bus: Rc<SimMessageBus> = Rc::new(SimMessageBus::new());

    let servers = vec![1, 2, 3];

    let replicas: Vec<_> = servers
        .iter()
        .map(|id| {
            Rc::new(RefCell::new(Replica::new(
                Config {
                    id: *id,
                    replicas: servers.clone(),
                },
                Rc::clone(&bus) as Rc<dyn MessageBus>,
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
    );

    for _ in 0..100 {
        sim.tick();
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::quickcheck;

    use super::*;

    quickcheck! {
      #[test]
      fn sim() -> bool {
        true
      }
    }
}
