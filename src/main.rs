use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    hash::Hash,
    rc::Rc,
};

use rand::{rngs::StdRng, Rng, SeedableRng};

type ReplicaId = u32;
type ProposalNumber = u64;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
enum RequestId {
    Prepare(ReplicaId, ProposalNumber),
    Accept(ReplicaId, ProposalNumber),
}

#[derive(Debug)]
struct Replica {
    min_proposal_number: ProposalNumber,
    accepted_proposal_number: Option<ProposalNumber>,
    accepted_value: Option<String>,

    config: Config,
    bus: Rc<dyn MessageBus>,

    inflight_requests: HashMap<RequestId, InflightRequest>,
}

#[derive(Debug)]
struct InflightRequest {
    proposal_number: ProposalNumber,
    proposed_value: Option<String>,
    responses: HashSet<PrepareOutput>,
}

#[derive(Debug)]
struct Config {
    id: ReplicaId,
    replicas: Vec<ReplicaId>,
}

#[derive(Debug, Clone)]
struct PrepareInput {
    from_replica_id: ReplicaId,
    request_id: RequestId,
    proposal_number: ProposalNumber,
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct PrepareOutput {
    from_replica_id: ReplicaId,
    request_id: RequestId,
    accepted_proposal_number: Option<ProposalNumber>,
    accepted_value: Option<String>,
}

#[derive(Debug, Clone)]
struct AcceptInput {
    from_replica_id: ReplicaId,
    request_id: RequestId,
    proposal_number: ProposalNumber,
    value: String,
}

#[derive(Debug)]
struct AcceptOutput {
    from_replica_id: ReplicaId,
    request_id: RequestId,
    min_proposal_number: ProposalNumber,
}

trait MessageBus: std::fmt::Debug {
    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput);
    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput);

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput);
    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput);
}

impl Replica {
    fn new(config: Config, bus: Rc<dyn MessageBus>) -> Self {
        Self {
            min_proposal_number: 0,
            accepted_proposal_number: None,
            accepted_value: None,
            config,
            bus,
            inflight_requests: HashMap::new(),
        }
    }

    fn majority(&self) -> usize {
        self.config.replicas.len() / 2 + 1
    }

    fn next_proposal_number(&mut self) -> u64 {
        self.min_proposal_number += 1;
        self.min_proposal_number
    }

    fn on_start_proposal(&mut self, value: String) {
        let proposal_number = self.next_proposal_number();
        self.broadcast_prepare(proposal_number, value);
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

    fn on_prepare_response(&mut self, input: PrepareOutput) {
        let majority = self.majority();
        let request_id = input.request_id;

        if let Some(req) = self.inflight_requests.get_mut(&request_id) {
            req.responses.insert(input);

            if req.responses.len() < majority {
                return;
            }

            let value = req
                .responses
                .iter()
                .filter(|response| response.accepted_proposal_number.is_some())
                .max_by_key(|response| response.accepted_proposal_number)
                .map(|response| response.accepted_value.clone().unwrap())
                .unwrap_or_else(|| req.proposed_value.clone().unwrap());

            let proposal_number = req.proposal_number;
            self.broadcast_accept(proposal_number, value);
            self.inflight_requests.remove(&request_id);
        }
    }

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

    fn on_accept_response(&mut self, input: AcceptOutput) {
        // TODO: clean up inflight requests.
    }

    fn broadcast_prepare(&mut self, proposal_number: ProposalNumber, value: String) {
        let request_id = RequestId::Prepare(self.config.id, proposal_number);

        self.inflight_requests.insert(
            request_id,
            InflightRequest {
                proposal_number,
                proposed_value: Some(value),
                responses: HashSet::new(),
            },
        );

        let input = PrepareInput {
            from_replica_id: self.config.id,
            request_id,
            proposal_number,
        };

        for i in 0..self.config.replicas.len() {
            let replica_id = self.config.replicas[i];

            if replica_id == self.config.id {
                self.on_prepare(input.clone());
            } else {
                self.bus.send_prepare(replica_id, input.clone());
            }
        }
    }

    fn broadcast_accept(&mut self, proposal_number: ProposalNumber, value: String) {
        let request_id = RequestId::Accept(self.config.id, proposal_number);

        self.inflight_requests.insert(
            request_id,
            InflightRequest {
                proposal_number,
                proposed_value: Some(value.clone()),
                responses: HashSet::new(),
            },
        );

        let input = AcceptInput {
            from_replica_id: self.config.id,
            request_id,
            proposal_number,
            value,
        };

        for i in 0..self.config.replicas.len() {
            let replica_id = self.config.replicas[i];

            if replica_id == self.config.id {
                self.on_accept(input.clone());
            } else {
                self.bus.send_accept(replica_id, input.clone());
            }
        }
    }
}

#[derive(Debug)]
struct Simulator {
    config: SimulatorConfig,
    rng: StdRng,
    replicas: Vec<Rc<RefCell<Replica>>>,
    bus: Rc<SimMessageBus>,
    num_client_requests_sent: u32,
}

#[derive(Debug)]
struct SimulatorConfig {
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
struct SimMessageBus {
    replicas: RefCell<Vec<Rc<RefCell<Replica>>>>,
    queue: RefCell<Vec<PendingMessage>>,
}

#[derive(Debug)]
enum PendingMessage {
    Prepare(ReplicaId, PrepareInput),
    PrepareResponse(ReplicaId, PrepareOutput),
    Accept(ReplicaId, AcceptInput),
    AcceptResponse(ReplicaId, AcceptOutput),
}

impl SimMessageBus {
    fn new() -> Self {
        Self {
            replicas: RefCell::new(Vec::new()),
            queue: RefCell::new(Vec::new()),
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

        dbg!(&message);
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
                let replica = self.find_replica(to_replica_id);
                replica.borrow_mut().on_accept_response(input);
            }
        }
    }
}

impl MessageBus for SimMessageBus {
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
        bus,
    );

    for _ in 0..100 {
        sim.tick();
    }
}

#[cfg(test)]
mod tests {
    use quickcheck::quickcheck;

    quickcheck! {
      #[test]
      fn sim() -> bool {
        true
      }
    }
}
