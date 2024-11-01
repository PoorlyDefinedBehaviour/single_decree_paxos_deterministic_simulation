use std::fmt::Display;

pub type ReplicaId = u32;
pub type ProposalNumber = u64;

#[derive(Debug, Hash, PartialEq, Eq, Clone, Copy)]
pub enum RequestId {
    Prepare(ReplicaId, ProposalNumber),
    Accept(ReplicaId, ProposalNumber),
}

impl Display for RequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RequestId::Prepare(replica_id, proposal_number) => {
                f.write_fmt(format_args!("RID(R{replica_id}, P{proposal_number})"))?;
            }
            RequestId::Accept(replica_id, proposal_number) => {
                f.write_fmt(format_args!("RID(R{replica_id}, P{proposal_number})"))?;
            }
        };
        std::fmt::Result::Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PrepareInput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub proposal_number: ProposalNumber,
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct PrepareOutput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub accepted_proposal_number: Option<ProposalNumber>,
    pub accepted_value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AcceptInput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub proposal_number: ProposalNumber,
    pub value: String,
}

#[derive(Debug, Hash, PartialEq, Eq, Clone)]
pub struct AcceptOutput {
    pub from_replica_id: ReplicaId,
    pub request_id: RequestId,
    pub min_proposal_number: ProposalNumber,
}
