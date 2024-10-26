use crate::types::{AcceptOutput, ReplicaId};

#[derive(Debug)]
pub(crate) struct Oracle {}

impl Oracle {
    pub fn new() -> Self {
        Self {}
    }

    pub fn on_proposal_accepted(&self, to_replica_id: ReplicaId, output: &AcceptOutput) {
        println!(
            "replica {} accepted proposal {output:?} from replica {to_replica_id}",
            output.from_replica_id
        );
    }
}
