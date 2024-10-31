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
    majority: usize,
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
    pub fn new(majority: usize, activity_log: Rc<RefCell<ActivityLog>>) -> Self {
        Self {
            activity_log,
            inflight_accept_requests: HashMap::new(),
            majority,
            decided_value: None,
        }
    }

    pub fn on_accept_sent(&mut self, to_replica_id: ReplicaId, input: &AcceptInput) {
        self.inflight_accept_requests.insert(
            input.request_id,
            InflightAcceptRequest::new(input.value.clone()),
        );
    }

    pub fn on_proposal_accepted(&mut self, to_replica_id: ReplicaId, output: &AcceptOutput) {
        if let Some(req) = self.inflight_accept_requests.get_mut(&output.request_id) {
            req.responses.insert(output.to_owned());
            if req.responses.len() < self.majority {
                return;
            }

            println!("aaaaa decided on {}", req.value);

            if self.decided_value.is_none() {
                self.decided_value = Some(req.value.clone());
            } else {
                assert_eq!(
                    self.decided_value.as_ref(),
                    Some(&req.value),
                    "majority of replicas decided on a different value after a value was accepted"
                );
            }

            self.activity_log.borrow_mut().record(format!(
                "[ORACLE] value accepted by majority of replicas: {}",
                self.decided_value.as_ref().unwrap()
            ));
        }
    }
}
