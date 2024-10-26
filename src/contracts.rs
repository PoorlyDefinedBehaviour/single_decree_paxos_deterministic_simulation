use crate::types::{AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ReplicaId};

pub(crate) trait MessageBus: std::fmt::Debug {
    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput);
    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput);

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput);
    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput);
}
