use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    rc::Rc,
};

use crate::{
    activity_log::ActivityLog,
    types::{AcceptInput, AcceptOutput, PrepareOutput, ReplicaId, RequestId},
};

#[derive(Debug)]
pub(crate) struct Oracle {
    activity_log: Rc<RefCell<ActivityLog>>,
    num_replicas: usize,
    // TODO: remove old requests.
    inflight_accept_requests: HashMap<RequestId, InflightAcceptRequest>,
    decided_value: Option<String>,
}

#[derive(Debug)]
struct InflightAcceptRequest {
    value: String,
    responses: HashSet<AcceptOutput>,
}

impl InflightAcceptRequest {
    fn new(value: String) -> Self {
        Self {
            value,
            responses: HashSet::new(),
        }
    }
}

impl Oracle {
    pub fn new(num_replicas: usize, activity_log: Rc<RefCell<ActivityLog>>) -> Self {
        Self {
            activity_log,
            inflight_accept_requests: HashMap::new(),
            num_replicas,
            decided_value: None,
        }
    }

    fn majority(&self) -> usize {
        self.num_replicas / 2 + 1
    }

    pub fn on_prepare_response_sent(&self, to_replica_id: ReplicaId, output: &PrepareOutput) {}

    pub fn on_accept_sent(&mut self, to_replica_id: ReplicaId, input: &AcceptInput) {
        self.inflight_accept_requests.insert(
            input.request_id,
            InflightAcceptRequest::new(input.value.clone()),
        );
    }

    pub fn on_proposal_accepted(&mut self, to_replica_id: ReplicaId, output: &AcceptOutput) {
        let majority = self.majority();

        if let Some(req) = self.inflight_accept_requests.get_mut(&output.request_id) {
            req.responses.insert(output.to_owned());
            if req.responses.len() < majority {
                return;
            }

            if self.decided_value.is_none() {
                self.decided_value = Some(req.value.clone());
            } else {
                assert_eq!(self.decided_value.as_ref(), Some(&req.value));
            }

            self.activity_log.borrow_mut().record(format!(
                "[ORACLE] value accepted by majority of replicas: {}",
                self.decided_value.as_ref().unwrap()
            ));
        }
    }
}
