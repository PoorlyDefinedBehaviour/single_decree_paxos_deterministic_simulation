use crate::types::{
    AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ProposalNumber, ReplicaId,
};

#[derive(Debug, Clone)]
pub struct DurableState {
    pub min_proposal_number: ProposalNumber,
    pub accepted_proposal_number: Option<ProposalNumber>,
    pub accepted_value: Option<String>,
}

pub trait MessageBus: std::fmt::Debug {
    fn send_start_proposal(&self, to_replica_id: ReplicaId, value: String);

    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput);
    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput);

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput);
    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput);
}

pub trait Storage: std::fmt::Debug {
    fn load(&self) -> DurableState;
    fn store(&self, state: &DurableState);
}
