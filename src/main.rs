#![feature(io_error_uncategorized)]
#![feature(io_error_more)]

use std::{
    collections::{HashMap, HashSet},
    rc::Rc,
};

use anyhow::Result;
use types::{
    AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ProposalNumber, ReplicaId, RequestId,
};

pub mod contracts;
pub mod file_storage;

#[cfg(test)]
pub mod simulation;
pub mod types;

#[derive(Debug)]
pub struct Replica {
    state: contracts::DurableState,

    config: Config,
    bus: Rc<dyn contracts::MessageBus>,
    storage: Rc<dyn contracts::Storage>,

    inflight_requests: HashMap<RequestId, InflightRequest>,
}

#[derive(Debug)]
pub struct InflightRequest {
    proposal_number: ProposalNumber,
    proposed_value: Option<String>,
    responses: HashSet<PrepareOutput>,
}

#[derive(Debug)]
pub struct Config {
    id: ReplicaId,
    replicas: Vec<ReplicaId>,
}

impl Replica {
    pub fn new(
        config: Config,
        bus: Rc<dyn contracts::MessageBus>,
        storage: Rc<dyn contracts::Storage>,
    ) -> Self {
        let state = storage.load();

        Self {
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

    fn next_proposal_number(&mut self) -> Result<u64> {
        let state = contracts::DurableState {
            min_proposal_number: self.state.min_proposal_number + 1,
            ..self.state.clone()
        };
        self.storage.store(&state)?;
        self.state = state;
        Ok(self.state.min_proposal_number)
    }

    pub fn on_start_proposal(&mut self, value: String) {
        let proposal_number = self.next_proposal_number().unwrap();
        self.broadcast_prepare(proposal_number, value);
    }

    pub fn on_prepare(&mut self, input: PrepareInput) {
        if input.proposal_number > self.state.min_proposal_number {
            let mut state = self.state.clone();
            state.min_proposal_number = input.proposal_number;
            self.storage.store(&state).unwrap();
            self.state = state;

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

    pub fn on_prepare_response(&mut self, input: PrepareOutput) {
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

    pub fn on_accept(&mut self, input: AcceptInput) {
        if input.proposal_number >= self.state.min_proposal_number {
            let mut state = self.state.clone();
            state.accepted_proposal_number = Some(input.proposal_number);
            state.accepted_value = Some(input.value);
            self.storage.store(&state).unwrap();
            self.state = state;

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

    pub fn on_accept_response(&mut self, _input: AcceptOutput) {
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

        for i in 0..self.config.replicas.len() {
            let replica_id = self.config.replicas[i];

            self.bus.send_accept(replica_id, input.clone());
        }
    }
}

fn main() {}
