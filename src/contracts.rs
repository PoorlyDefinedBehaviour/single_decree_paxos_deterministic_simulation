use std::path::Path;

use crate::types::{
    AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ProposalNumber, ReplicaId,
};

#[derive(Debug, PartialEq, Clone, serde::Deserialize, serde::Serialize)]
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
    fn store(&self, state: &DurableState) -> std::io::Result<()>;
}

pub trait FileSystem {
    fn create_dir_all(&self, path: &Path) -> std::io::Result<()>;
    fn open(&self, path: &Path, options: OpenOptions) -> std::io::Result<Box<dyn File>>;
    fn rename(&self, from: &Path, to: &Path) -> std::io::Result<()>;
}

#[derive(Debug)]
pub struct OpenOptions {
    pub create: bool,
    pub write: bool,
    pub read: bool,
    pub truncate: bool,
}

#[derive(Debug)]
pub struct Metadata {
    pub len: u64,
}

impl Metadata {
    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

pub trait File: std::io::Read + std::io::Write {
    fn metadata(&self) -> std::io::Result<Metadata>;
}
