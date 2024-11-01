use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use types::{
    AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ProposalNumber, ReplicaId, RequestId,
};

mod activity_log;
mod contracts;
mod in_memory_storage;
mod oracle;
mod simulation;
mod types;

#[derive(Debug)]
struct Replica {
    state: contracts::DurableState,
    next_proposal_number: ProposalNumber,

    config: Config,
    bus: Rc<dyn contracts::MessageBus>,
    storage: Rc<dyn contracts::Storage>,

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

impl Replica {
    fn new(
        config: Config,
        bus: Rc<dyn contracts::MessageBus>,
        storage: Rc<dyn contracts::Storage>,
    ) -> Self {
        let state = storage.load();

        Self {
            next_proposal_number: state.min_proposal_number + 1,
            state,
            config,
            bus,
            storage,
            inflight_requests: HashMap::new(),
        }
    }

    fn majority(&self) -> usize {
        self.config.replicas.len() / 2 + 1
    }

    fn next_proposal_number(&mut self) -> u64 {
        let proposal_number = self.next_proposal_number;
        self.next_proposal_number += 1;
        proposal_number
    }

    fn on_start_proposal(&mut self, value: String) {
        let proposal_number = self.next_proposal_number();
        self.broadcast_prepare(proposal_number, value);
    }

    fn on_prepare(&mut self, input: PrepareInput) {
        if input.proposal_number > self.state.min_proposal_number {
            self.state.min_proposal_number = input.proposal_number;
            self.storage.store(&self.state);
            eprintln!(
                "on_prepare: replica={} state={:?} input={:?}",
                self.config.id, &self.state, &input
            );
            self.bus.send_prepare_response(
                input.from_replica_id,
                PrepareOutput {
                    from_replica_id: self.config.id,
                    request_id: input.request_id,
                    accepted_proposal_number: self.state.accepted_proposal_number,
                    accepted_value: self.state.accepted_value.clone(),
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

            eprintln!(
                "on_prepare_response: replica={} state={:?} responses={:?}",
                self.config.id, &self.state, &req.responses
            );

            let proposal_number = req.proposal_number;
            self.broadcast_accept(proposal_number, value);
            self.inflight_requests.remove(&request_id);
        }
    }

    fn on_accept(&mut self, input: AcceptInput) {
        if input.proposal_number >= self.state.min_proposal_number {
            self.state.accepted_proposal_number = Some(input.proposal_number);
            self.state.accepted_value = Some(input.value.clone());
            self.storage.store(&self.state);

            eprintln!(
                "on_accept: replica={} state={:?} input={:?}",
                self.config.id, &self.state, &input
            );

            self.bus.send_accept_response(
                input.from_replica_id,
                AcceptOutput {
                    from_replica_id: self.config.id,
                    request_id: input.request_id,
                    min_proposal_number: self.state.min_proposal_number,
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

            self.bus.send_prepare(replica_id, input.clone());
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

        eprintln!(
            "broadcast_accept: replica={} state={:?} input={:?}",
            self.config.id, &self.state, &input
        );

        for i in 0..self.config.replicas.len() {
            let replica_id = self.config.replicas[i];

            self.bus.send_accept(replica_id, input.clone());
        }
    }
}

fn main() {}
