pub(crate) type ReplicaId = u32;
pub(crate) type ProposalNumber = u64;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub(crate) enum RequestId {
    Prepare(ReplicaId, ProposalNumber),
    Accept(ReplicaId, ProposalNumber),
}

#[derive(Debug, Clone)]
pub(crate) struct PrepareInput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub proposal_number: ProposalNumber,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub(crate) struct PrepareOutput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub accepted_proposal_number: Option<ProposalNumber>,
    pub accepted_value: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AcceptInput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub proposal_number: ProposalNumber,
    pub value: String,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub(crate) struct AcceptOutput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub min_proposal_number: ProposalNumber,
}
