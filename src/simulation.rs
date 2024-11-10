#![allow(clippy::unit_cmp)]

use core::iter::Iterator;
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
    panic::UnwindSafe,
    path::PathBuf,
    rc::Rc,
};

use rand::{
    rngs::StdRng,
    seq::{IteratorRandom, SliceRandom},
    Rng,
};

use crate::{
    activity_log::ActivityLog,
    contracts::{self, FileSystem, MessageBus},
    oracle::Oracle,
    types::{AcceptInput, AcceptOutput, PrepareInput, PrepareOutput, ReplicaId},
    Replica,
};

#[derive(Debug)]
struct ActionSimulator {
    config: ActionSimulatorConfig,
    metrics: ActionSimulatorMetrics,
    action_set: Vec<Action>,
    rng: Rc<RefCell<StdRng>>,
    bus: Rc<SimMessageBus>,
    replicas: Vec<Replica>,
    oracle: Oracle,
    activity_log: Rc<RefCell<ActivityLog>>,
    healthy_replicas: HashSet<ReplicaId>,
    failed_replicas: HashSet<ReplicaId>,
}

impl UnwindSafe for ActionSimulator {}

#[derive(Debug)]
pub struct ActionSimulatorConfig {
    max_actions: u32,
    max_user_requests: u32,
    max_replica_crashes: u32,
    max_replica_restarts: u32,
}

#[derive(Debug)]
struct ActionSimulatorMetrics {
    num_user_requests_sent: u32,
    num_replica_crashes: u32,
    num_replica_restarts: u32,
}

impl ActionSimulatorMetrics {
    fn new() -> Self {
        Self {
            num_user_requests_sent: 0,
            num_replica_crashes: 0,
            num_replica_restarts: 0,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
enum Action {
    SendUserRequest,
    CrashReplica,
    RestartReplica,
    DeliverMessage,
    DropMessage,
    DuplicateMessage,
}

impl Action {
    fn can_be_scheduled(
        &self,
        config: &ActionSimulatorConfig,
        metrics: &ActionSimulatorMetrics,
        num_messages_in_flight: usize,
    ) -> bool {
        match self {
            Action::SendUserRequest => metrics.num_user_requests_sent < config.max_user_requests,
            Action::CrashReplica => true,
            Action::RestartReplica => true,
            Action::DeliverMessage => num_messages_in_flight > 0,
            Action::DropMessage => num_messages_in_flight > 0,
            Action::DuplicateMessage => num_messages_in_flight > 0,
        }
    }
}

impl ActionSimulator {
    fn new(
        config: ActionSimulatorConfig,
        rng: Rc<RefCell<StdRng>>,
        replicas: Vec<Replica>,
        bus: Rc<SimMessageBus>,
        oracle: Oracle,
        activity_log: Rc<RefCell<ActivityLog>>,
    ) -> Self {
        Self {
            config,
            rng,
            bus,
            action_set: vec![
                Action::SendUserRequest,
                Action::CrashReplica,
                Action::RestartReplica,
                Action::DeliverMessage,
                Action::DropMessage,
                Action::DuplicateMessage,
            ],
            oracle,
            activity_log,
            healthy_replicas: HashSet::from_iter(replicas.iter().map(|r| r.config.id)),
            failed_replicas: HashSet::new(),
            replicas,
            metrics: ActionSimulatorMetrics::new(),
        }
    }

    fn majority(&self) -> usize {
        self.replicas.len() / 2 + 1
    }

    fn next_action(&mut self) -> Action {
        let action = self
            .action_set
            .iter()
            .filter(|action| {
                action.can_be_scheduled(
                    &self.config,
                    &self.metrics,
                    self.bus.num_messages_in_flight(),
                )
            })
            .choose(unsafe { &mut *self.rng.as_ptr() })
            .cloned()
            .unwrap();

        match action {
            Action::SendUserRequest => {
                self.metrics.num_user_requests_sent += 1;
            }
            Action::CrashReplica => {
                self.metrics.num_replica_crashes += 1;
            }
            Action::RestartReplica => {
                self.metrics.num_replica_restarts += 1;
            }
            _ => {
                // noop.
            }
        }

        action
    }

    fn choose_healthy_replica(&mut self) -> &Replica {
        let replicas = self
            .replicas
            .iter()
            .filter(|r| self.healthy_replicas.contains(&r.config.id));
        replicas.choose(unsafe { &mut *self.rng.as_ptr() }).unwrap()
    }

    fn choose_any_replica(&mut self) -> &Replica {
        self.replicas
            .choose(unsafe { &mut *self.rng.as_ptr() })
            .unwrap()
    }

    fn get_healthy_replicas_mut(&mut self) -> Vec<&mut Replica> {
        self.replicas
            .iter_mut()
            .filter(|r| self.healthy_replicas.contains(&r.config.id))
            .collect()
    }

    fn get_healthy_replica_index(&mut self, replica_id: ReplicaId) -> Option<usize> {
        self.replicas.iter().enumerate().find_map(|(i, r)| {
            if r.config.id == replica_id && self.healthy_replicas.contains(&r.config.id) {
                Some(i)
            } else {
                None
            }
        })
    }

    fn recreate_replica(&mut self, replica_id: ReplicaId) {
        for i in 0..self.replicas.len() {
            if self.replicas[i].config.id == replica_id {
                let replica = self.replicas.swap_remove(i);
                self.replicas
                    .push(Replica::new(replica.config, replica.bus, replica.storage));
                return;
            }
        }
    }

    fn run(&mut self) {
        for i in 0..self.config.max_actions {
            let action = self.next_action();

            match action {
                Action::SendUserRequest => {
                    let replica_id = {
                        let replica = self.choose_healthy_replica();
                        replica.config.id
                    };
                    let value = format!("V({}, {})", replica_id, i);
                    self.bus.send_start_proposal(replica_id, value);
                }
                Action::CrashReplica => {
                    if self.healthy_replicas.len() > self.majority() {
                        let replica_id = {
                            let replica = self.choose_healthy_replica();
                            replica.config.id
                        };
                        self.healthy_replicas.remove(&replica_id);
                        self.failed_replicas.insert(replica_id);
                        self.activity_log
                            .borrow_mut()
                            .record(format!("[SIMULATOR] CRASH Replica({replica_id})"));
                    }
                }
                Action::RestartReplica => {
                    let replica_id = {
                        let replica = self.choose_any_replica();
                        replica.config.id
                    };
                    self.recreate_replica(replica_id);
                    self.failed_replicas.remove(&replica_id);
                    self.healthy_replicas.insert(replica_id);
                    self.activity_log
                        .borrow_mut()
                        .record(format!("[SIMULATOR] RESTART Replica({replica_id})"));
                }
                Action::DropMessage => {
                    if let Some(message) = self.bus.next_message() {
                        self.activity_log
                            .borrow_mut()
                            .record(message.to_activity_log_event(EventType::Drop));
                    }
                }
                Action::DuplicateMessage => {
                    if let Some(message) = self.bus.next_message() {
                        self.activity_log
                            .borrow_mut()
                            .record(message.to_activity_log_event(EventType::Duplicate));
                        self.bus.add_message(message.clone());
                        self.bus.add_message(message);
                    }
                }
                Action::DeliverMessage => {
                    if let Some(message) = self.bus.next_message() {
                        self.deliver_message(message);
                    }
                }
            }
        }

        while let Some(message) = self.bus.next_message() {
            self.deliver_message(message);
        }
    }

    fn deliver_message(&mut self, message: PendingMessage) {
        let Some(i) = self.get_healthy_replica_index(message.get_to_replica_id()) else {
            return;
        };

        let replica = &mut self.replicas[i];

        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Receive));

        match message {
            PendingMessage::StartProposal(_to_replica_id, value) => {
                replica.on_start_proposal(value);
            }
            PendingMessage::Prepare(_to_replica_id, input) => {
                replica.on_prepare(input);
            }
            PendingMessage::PrepareResponse(_to_replica_id, input) => {
                replica.on_prepare_response(input);
            }
            PendingMessage::Accept(to_replica_id, input) => {
                replica.on_accept(input);
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                replica.on_accept_response(input);
            }
        }
    }
}

#[derive(Debug)]
pub struct SimMessageBus {
    queue: RefCell<MessageQueue>,
    oracle: RefCell<Oracle>,
    activity_log: Rc<RefCell<ActivityLog>>,
}

#[derive(Debug)]
struct MessageQueue {
    rng: Rc<RefCell<StdRng>>,
    items: Vec<PendingMessage>,
}

impl MessageQueue {
    fn new(rng: Rc<RefCell<StdRng>>) -> Self {
        Self {
            rng,
            items: Vec::new(),
        }
    }

    fn push(&mut self, message: PendingMessage) {
        self.items.push(message);
    }

    // Pops a message from the queue. Messages may be delivered in any order.
    fn pop(&mut self) -> Option<PendingMessage> {
        if self.items.is_empty() {
            return None;
        }

        let i = self
            .rng
            .as_ref()
            .borrow_mut()
            .gen_range(0..self.items.len());
        let item = self.items.swap_remove(i);
        Some(item)
    }
}

#[derive(Debug, Clone)]
pub enum PendingMessage {
    StartProposal(ReplicaId, String),
    Prepare(ReplicaId, PrepareInput),
    PrepareResponse(ReplicaId, PrepareOutput),
    Accept(ReplicaId, AcceptInput),
    AcceptResponse(ReplicaId, AcceptOutput),
}

#[derive(Debug)]
enum EventType {
    Queue,
    Receive,
    Drop,
    Duplicate,
}

impl PendingMessage {
    fn get_to_replica_id(&self) -> ReplicaId {
        match self {
            PendingMessage::StartProposal(to_replica_id, _) => *to_replica_id,
            PendingMessage::Prepare(to_replica_id, prepare_input) => *to_replica_id,
            PendingMessage::PrepareResponse(to_replica_id, prepare_output) => *to_replica_id,
            PendingMessage::Accept(to_replica_id, accept_input) => *to_replica_id,
            PendingMessage::AcceptResponse(to_replica_id, accept_output) => *to_replica_id,
        }
    }
    fn to_activity_log_event(&self, event_type: EventType) -> String {
        match self {
            PendingMessage::StartProposal(to_replica_id, value) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Simulator -> Replica({}) QUEUED StartProposal({}) ",
                    to_replica_id, value,
                ),
                EventType::Receive => format!(
                    "[BUS] Simulator -> Replica({}) RECEIVED StartProposal({})",
                    to_replica_id, value
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Simulator -> Replica({}) RECEIVED StartProposal({})",
                    to_replica_id, value
                ),
                EventType::Duplicate => format!(
                  "[SIMULATOR] DUPLICATE Simulator -> Replica({}) RECEIVED StartProposal({})",
                  to_replica_id, value
              ),
            },
            PendingMessage::Prepare(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) Prepare({})",
                    msg.from_replica_id, to_replica_id, msg.request_id,
                ),
                EventType::Duplicate => format!(
                  "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) Prepare({})",
                  msg.from_replica_id, to_replica_id, msg.request_id,
              ),
            },

            PendingMessage::PrepareResponse(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED PrepareResponse({}, {:?}, {:?})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED PrepareResponse({}, {:?}, {:?})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) PrepareResponse({}, {:?}, {:?})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.accepted_proposal_number,
                    msg.accepted_value,
                ),
                EventType::Duplicate => format!(
                  "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) PrepareResponse({}, {:?}, {:?})",
                  msg.from_replica_id,
                  to_replica_id,
                  msg.request_id,
                  msg.accepted_proposal_number,
                  msg.accepted_value,
              ),
            },
            PendingMessage::Accept(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) Accept({}, {}, {})",
                    msg.from_replica_id,
                    to_replica_id,
                    msg.request_id,
                    msg.proposal_number,
                    msg.value,
                ),
                EventType::Duplicate => format!(
                  "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) Accept({}, {}, {})",
                  msg.from_replica_id,
                  to_replica_id,
                  msg.request_id,
                  msg.proposal_number,
                  msg.value,
              ),
            },
            PendingMessage::AcceptResponse(to_replica_id, msg) => match event_type {
                EventType::Queue => format!(
                    "[BUS] Replica({}) -> Replica({}) QUEUED AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number
                ),
                EventType::Receive => format!(
                    "[BUS] Replica({}) -> Replica({}) RECEIVED AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number,
                ),
                EventType::Drop => format!(
                    "[SIMULATOR] DROP Replica({}) -> Replica({}) AcceptResponse({}, {})",
                    msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number,
                ),
                EventType::Duplicate => format!(
                  "[SIMULATOR] DUPLICATE Replica({}) -> Replica({}) AcceptResponse({}, {})",
                  msg.from_replica_id, to_replica_id, msg.request_id, msg.min_proposal_number,
              ),
            },
        }
    }
}

impl SimMessageBus {
    fn new(
        rng: Rc<RefCell<StdRng>>,
        oracle: Oracle,
        activity_log: Rc<RefCell<ActivityLog>>,
    ) -> Self {
        Self {
            queue: RefCell::new(MessageQueue::new(Rc::clone(&rng))),

            oracle: RefCell::new(oracle),
            activity_log,
        }
    }

    fn find_replica<'a>(
        &self,
        replicas: &'a mut [Replica],
        replica_id: ReplicaId,
    ) -> Option<&'a mut Replica> {
        replicas.iter_mut().find(|r| r.config.id == replica_id)
    }

    fn next_message(&self) -> Option<PendingMessage> {
        let message = self.queue.borrow_mut().pop()?;
        match &message {
            PendingMessage::Accept(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_accept_sent(*to_replica_id, input);
            }
            PendingMessage::AcceptResponse(to_replica_id, input) => {
                self.oracle
                    .borrow_mut()
                    .on_proposal_accepted(*to_replica_id, input);
            }
            PendingMessage::StartProposal(_, _)
            | PendingMessage::Prepare(_, _)
            | PendingMessage::PrepareResponse(_, _) => {
                // no-op.
            }
        }
        Some(message)
    }

    fn add_message(&self, message: PendingMessage) {
        let mut queue = self.queue.borrow_mut();
        queue.push(message);
    }

    fn num_messages_in_flight(&self) -> usize {
        self.queue.borrow().items.len()
    }
}

impl contracts::MessageBus for SimMessageBus {
    fn send_start_proposal(&self, to_replica_id: ReplicaId, value: String) {
        let message = PendingMessage::StartProposal(to_replica_id, value);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare(&self, to_replica_id: ReplicaId, input: PrepareInput) {
        let message = PendingMessage::Prepare(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_prepare_response(&self, to_replica_id: ReplicaId, input: PrepareOutput) {
        let message = PendingMessage::PrepareResponse(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept(&self, to_replica_id: ReplicaId, input: AcceptInput) {
        let message = PendingMessage::Accept(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }

    fn send_accept_response(&self, to_replica_id: ReplicaId, input: AcceptOutput) {
        let message = PendingMessage::AcceptResponse(to_replica_id, input);
        self.activity_log
            .as_ref()
            .borrow_mut()
            .record(message.to_activity_log_event(EventType::Queue));
        self.queue.borrow_mut().push(message);
    }
}

struct SimFileSystem {
    dirs: Rc<RefCell<HashSet<PathBuf>>>,
    cache: Rc<RefCell<HashMap<PathBuf, Rc<RefCell<FakeFile>>>>>,
    disk: Rc<RefCell<HashMap<PathBuf, Vec<u8>>>>,
}

#[derive(Debug)]
struct FakeFile {
    path: PathBuf,
    data: Vec<u8>,
}

impl SimFileSystem {
    fn new() -> Self {
        let fs = SimFileSystem {
            dirs: Rc::new(RefCell::new(HashSet::new())),
            cache: Rc::new(RefCell::new(HashMap::new())),
            disk: Rc::new(RefCell::new(HashMap::new())),
        };
        fs.create_dir_all(&PathBuf::from(".")).unwrap();
        fs
    }
}

struct SimFile {
    cache: Rc<RefCell<HashMap<PathBuf, Rc<RefCell<FakeFile>>>>>,
    disk: Rc<RefCell<HashMap<PathBuf, Vec<u8>>>>,
    position: usize,
    open_options: contracts::OpenOptions,
    file: Rc<RefCell<FakeFile>>,
}

impl contracts::FileSystem for SimFileSystem {
    fn create_dir_all(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut dirs = self.dirs.borrow_mut();

        let mut current_path: PathBuf = PathBuf::new();

        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    current_path = current_path.join(std::path::Component::RootDir.as_os_str());
                }
                std::path::Component::ParentDir => {
                    continue;
                }
                std::path::Component::CurDir => {
                    current_path = current_path.join(".");

                    if !dirs.contains(&current_path) {
                        dirs.insert(current_path.clone());
                    }
                }
                std::path::Component::Normal(dir) => {
                    current_path = current_path.join(dir);

                    if !dirs.contains(&current_path) {
                        dirs.insert(current_path.clone());
                    }
                }
            }
        }

        Ok(())
    }

    fn open(
        &self,
        path: &std::path::Path,
        options: contracts::OpenOptions,
    ) -> std::io::Result<Box<dyn contracts::File>> {
        if !self
            .dirs
            .borrow()
            .contains(&PathBuf::from(path.parent().unwrap()))
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No such file or directory",
            ));
        }

        let mut cache = self.cache.borrow_mut();

        if options.create && !cache.contains_key(path) {
            cache.insert(
                path.to_owned(),
                Rc::new(RefCell::new(FakeFile {
                    path: path.to_owned(),
                    data: Vec::new(),
                })),
            );
        }

        match cache.get_mut(path) {
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No such file or directory",
            )),
            Some(file) => {
                if options.truncate {
                    let mut file = file.borrow_mut();
                    file.data = Vec::new();
                }

                Ok(Box::new(SimFile::new(
                    Rc::clone(&self.cache),
                    Rc::clone(&self.disk),
                    options,
                    Rc::clone(file),
                )))
            }
        }
    }

    fn rename(&self, from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = to.parent() {
            if !self.dirs.borrow().contains(&PathBuf::from(parent)) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No such file or directory",
                ));
            }
        }

        let mut cache = self.cache.borrow_mut();

        let Some(file) = cache.remove(from) else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No such file or directory",
            ));
        };

        file.borrow_mut().path = to.to_owned();
        cache.insert(to.to_owned(), file);

        Ok(())
    }
}

impl SimFile {
    fn new(
        cache: Rc<RefCell<HashMap<PathBuf, Rc<RefCell<FakeFile>>>>>,
        disk: Rc<RefCell<HashMap<PathBuf, Vec<u8>>>>,
        open_options: contracts::OpenOptions,
        file: Rc<RefCell<FakeFile>>,
    ) -> Self {
        Self {
            cache,
            disk,
            position: 0,
            open_options,
            file,
        }
    }
}

impl std::io::Read for SimFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.open_options.read {
            return Err(std::io::Error::new(std::io::ErrorKind::Uncategorized, ""));
        }

        let file = self.file.borrow();

        if self.position >= file.data.len() {
            return Ok(0);
        }

        let num_bytes_to_read =
            std::cmp::min(file.data.len().saturating_sub(self.position), buf.len());

        #[allow(clippy::manual_memcpy)]
        for i in 0..num_bytes_to_read {
            buf[i] = file.data[self.position + i];
        }

        self.position += num_bytes_to_read;
        Ok(num_bytes_to_read)
    }
}

impl std::io::Write for SimFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if !self.open_options.write {
            return Err(std::io::Error::new(std::io::ErrorKind::Uncategorized, ""));
        }

        let mut file = self.file.borrow_mut();

        if file.data.len() < self.position + buf.len() {
            file.data.resize(self.position + buf.len(), 0);
        }

        for (i, byte) in buf.iter().enumerate() {
            file.data[self.position + i] = *byte;
        }
        self.position += buf.len();

        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let file = self.file.borrow();

        if !self.cache.borrow().contains_key(&file.path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Uncategorized,
                "Bad file descriptor",
            ));
        }

        let mut disk = self.disk.borrow_mut();

        disk.insert(file.path.clone(), file.data.clone());

        Ok(())
    }
}

impl contracts::File for SimFile {
    fn metadata(&self) -> std::io::Result<contracts::Metadata> {
        let file = self.file.borrow();
        Ok(contracts::Metadata {
            len: file.data.len() as u64,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io::{Read, Write},
        path::PathBuf,
    };

    use anyhow::Result;

    use quickcheck::quickcheck;
    use rand::{Rng, SeedableRng};
    use uuid::Uuid;

    use crate::{file_storage::FileStorage, Config, Replica};

    use super::*;

    #[test]
    fn action_simulation() -> Result<()> {
        let count = if std::env::var("SEED").is_ok() {
            1
        } else {
            std::env::var("MAX_THREADS")
                .map(|v| v.parse::<usize>().unwrap())
                .unwrap_or_else(|_| std::thread::available_parallelism().unwrap().get())
        };

        let max_iters = std::env::var("MAX_ITERS")
            .map(|v| v.parse::<u64>().unwrap())
            .unwrap_or_else(|_| 10_000);

        let max_actions = std::env::var("MAX_ACTIONS")
            .map(|v| v.parse::<u32>().unwrap())
            .unwrap_or_else(|_| 100);

        eprintln!("Spawning {count} threads");

        let handles = (0..count)
            .map(|thread_id| {
                std::thread::Builder::new()
                    .name(format!("Thread({thread_id})"))
                    .spawn(move || {
                        let seed: u64 = std::env::var("SEED")
                            .map(|v| v.parse::<u64>().unwrap())
                            .unwrap_or_else(|_| rand::thread_rng().gen());

                        eprintln!("SEED={seed}");

                        let rng = Rc::new(RefCell::new(rand::rngs::StdRng::seed_from_u64(seed)));

                        for i in 0..max_iters {
                            if i % 1_000 == 0 {
                                eprintln!("Thread({thread_id}) Running simulation {i}");
                            }

                            let simulator_config = {
                                let mut rng = rng.borrow_mut();
                                ActionSimulatorConfig {
                                    max_actions,
                                    max_user_requests: rng.gen::<u32>() % 100 + 1,
                                    max_replica_crashes: rng.gen::<u32>() % 100,
                                    max_replica_restarts: rng.gen::<u32>() % 100,
                                }
                            };

                            let activity_log = Rc::new(RefCell::new(ActivityLog::new()));

                            let servers = vec![1, 2, 3];

                            let majority = servers.len() / 2 + 1;

                            let bus: Rc<SimMessageBus> = Rc::new(SimMessageBus::new(
                                Rc::clone(&rng),
                                Oracle::new(majority, Rc::clone(&activity_log)),
                                Rc::clone(&activity_log),
                            ));

                            let replicas: Vec<_> = servers
                                .iter()
                                .map(|id| {
                                    Replica::new(
                                        Config {
                                            id: *id,
                                            replicas: servers.clone(),
                                        },
                                        Rc::clone(&bus) as Rc<dyn contracts::MessageBus>,
                                        Rc::new(
                                            FileStorage::new(
                                                Rc::new(SimFileSystem::new()),
                                                PathBuf::from("dir"),
                                            )
                                            .unwrap(),
                                        ),
                                    )
                                })
                                .collect();

                            let mut sim = ActionSimulator::new(
                                simulator_config,
                                Rc::clone(&rng),
                                replicas,
                                Rc::clone(&bus),
                                Oracle::new(majority, Rc::clone(&activity_log)),
                                Rc::clone(&activity_log),
                            );

                            let result = std::panic::catch_unwind(move || {
                                sim.run();
                                assert!(sim.bus.queue.borrow().items.is_empty());
                            });
                            if result.is_err() {
                                activity_log.borrow_mut().print_events();
                                eprintln!("SEED={seed}");
                                std::process::exit(1);
                            }
                        }
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;

        for handle in handles {
            handle.join().unwrap();
        }

        Ok(())
    }

    #[derive(Debug, Clone)]
    enum FileSystemOp {
        CreateDirAll(PathBuf),
        Open(PathBuf, bool, bool, bool, bool),
        Read(usize, usize),
        Write(usize, String),
        Rename(PathBuf, PathBuf),
        Metadata(usize),
    }

    impl quickcheck::Arbitrary for FileSystemOp {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            let mut path = PathBuf::from("");
            let max_depth = u8::arbitrary(g) % 5;
            for i in 1..max_depth {
                path = path.join(i.to_string());
            }

            if bool::arbitrary(g) {
                FileSystemOp::CreateDirAll(path)
            } else if bool::arbitrary(g) {
                let create = bool::arbitrary(g);

                let truncate = bool::arbitrary(g);
                // Create and Truncate requires write.
                let mut write = create || truncate || bool::arbitrary(g);

                let mut read = bool::arbitrary(g);

                // At least one of read or write must be true.
                if !write && !read {
                    if bool::arbitrary(g) {
                        write = true;
                    } else {
                        read = true;
                    }
                }
                FileSystemOp::Open(path, create, write, read, truncate)
            } else if bool::arbitrary(g) {
                let mut new_path = PathBuf::from("");
                let max_depth = u8::arbitrary(g) % 5;
                for i in 1..max_depth {
                    new_path = new_path.join(i.to_string());
                }
                FileSystemOp::Rename(path, new_path)
            } else if bool::arbitrary(g) {
                FileSystemOp::Metadata(usize::arbitrary(g))
            } else if bool::arbitrary(g) {
                FileSystemOp::Read(usize::arbitrary(g), usize::arbitrary(g) % 1024)
            } else {
                FileSystemOp::Write(usize::arbitrary(g), String::arbitrary(g))
            }
        }
    }

    fn check_sim_file_system(ops: Vec<FileSystemOp>) -> bool {
        let fs = SimFileSystem::new();
        let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut files = Vec::new();

        for op in ops {
            match op {
                FileSystemOp::CreateDirAll(path) => {
                    let path = dir.join(path);
                    std::fs::create_dir_all(&path).unwrap();
                    fs.create_dir_all(&path).unwrap();
                }
                FileSystemOp::Open(path, create, write, read, truncate) => {
                    let path = dir.join(path).join("filename");

                    let model_result = std::fs::OpenOptions::new()
                        .create(create)
                        .write(write)
                        .read(read)
                        .truncate(truncate)
                        .open(&path);

                    let result = fs.open(
                        &path,
                        contracts::OpenOptions {
                            create,
                            write,
                            read,
                            truncate,
                        },
                    );

                    if model_result.is_err() {
                        assert_eq!(
                            model_result.err().map(|err| err.kind()),
                            result.err().map(|err| err.kind())
                        );
                    } else {
                        assert!(result.is_ok(), "path={path:?} {:?}", result.err());
                        files.push((model_result.unwrap(), result.unwrap()));
                    }
                }
                FileSystemOp::Read(i, buffer_size) => {
                    if files.is_empty() {
                        continue;
                    }

                    let i = i % files.len();
                    let f = &mut files[i];

                    let mut model_buffer = vec![0_u8; buffer_size];
                    let model_result = f.0.read(&mut model_buffer);

                    let mut buffer = vec![0_u8; buffer_size];
                    let result = f.1.read(&mut buffer);

                    if model_result.is_err() {
                        assert_eq!(
                            model_result.err().map(|err| err.kind()),
                            result.err().map(|err| err.kind())
                        );
                    } else {
                        assert!(result.is_ok());
                        assert_eq!(
                            model_buffer[0..model_result.unwrap()],
                            buffer[0..result.unwrap()]
                        );
                    }
                }
                FileSystemOp::Write(i, data) => {
                    if files.is_empty() {
                        continue;
                    }

                    let i = i % files.len();
                    let f = &mut files[i];

                    let model_result = f.0.write_all(data.as_ref());
                    let result = f.1.write_all(data.as_ref());

                    if model_result.is_err() {
                        assert_eq!(
                            model_result.err().map(|err| err.kind()),
                            result.err().map(|err| err.kind())
                        );
                    } else {
                        assert_eq!(model_result.unwrap(), result.unwrap());
                    }
                }
                FileSystemOp::Rename(from, to) => {
                    let from_path = dir.join(from).join("filename");
                    let to_path = dir.join(to).join("filename");
                    let model_result = std::fs::rename(&from_path, &to_path);
                    let result = fs.rename(&from_path, &to_path);

                    // .err() returns None when the result is an Ok().
                    assert_eq!(
                        model_result.err().map(|err| err.kind()),
                        result.err().map(|err| err.kind())
                    );
                }
                FileSystemOp::Metadata(i) => {
                    if files.is_empty() {
                        continue;
                    }

                    let i = i % files.len();
                    let f = &mut files[i];

                    let model_result = f.0.metadata();
                    let result = f.1.metadata();

                    if model_result.is_err() {
                        assert_eq!(
                            model_result.err().map(|err| err.kind()),
                            result.err().map(|err| err.kind())
                        );
                    } else {
                        assert_eq!(model_result.unwrap().len(), result.unwrap().len());
                    }
                }
            }
        }

        true
    }

    quickcheck! {
      #[test]
      fn test_sim_file_system(ops: Vec<FileSystemOp>) -> bool {
        check_sim_file_system(ops)
      }
    }

    #[test]
    fn test_sim_file_system_1() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1")),
            FileSystemOp::Open(PathBuf::from("1"), true, true, false, false),
        ]));
    }

    #[test]
    fn test_sim_file_system_2() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1")),
            FileSystemOp::Open(PathBuf::from(""), true, true, false, false),
            FileSystemOp::Read(10681956722829677122, 278)
        ]));
    }

    #[test]
    fn test_sim_file_system_3() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("")),
            FileSystemOp::Open(PathBuf::from(""), false, true, false, false)
        ]));
    }

    #[test]
    fn test_sim_file_system_4() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1")),
            FileSystemOp::Open(PathBuf::from(""), true, true, false, false),
            FileSystemOp::Read(10455096010292380886, 99)
        ]));
    }

    #[test]
    fn test_sim_file_system_5() {
        assert!(check_sim_file_system(vec![FileSystemOp::Rename(
            PathBuf::from(""),
            PathBuf::from("")
        )]));
    }

    #[test]
    fn test_sim_file_system_6() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1/2")),
            FileSystemOp::Open(PathBuf::from(""), true, true, false, true),
            FileSystemOp::Open(PathBuf::from("1"), true, true, false, true),
            FileSystemOp::Open(PathBuf::from(""), false, false, true, false),
            FileSystemOp::Open(PathBuf::from(""), false, false, true, false),
            FileSystemOp::Rename(PathBuf::from(""), PathBuf::from("1")),
            FileSystemOp::Read(18207958439361410334, 706)
        ]));
    }

    #[test]
    fn test_sim_file_system_7() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1/2/3")),
            FileSystemOp::Open(PathBuf::from("1/2"), true, true, false, false),
            FileSystemOp::Open(PathBuf::from("1"), true, true, false, true),
            FileSystemOp::Open(PathBuf::from("1/2"), false, true, true, true),
            FileSystemOp::Rename(PathBuf::from("1"), PathBuf::from("1/2")),
            FileSystemOp::Rename(PathBuf::from("1/2"), PathBuf::from("1"))
        ]));
    }

    #[test]
    fn test_sim_file_system_8() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1/2/3")),
            FileSystemOp::Open(PathBuf::from("1"), true, true, true, false),
            FileSystemOp::Rename(PathBuf::from("1"), PathBuf::from("1/2")),
            FileSystemOp::Metadata(4799043926653059155)
        ]));
    }

    #[test]
    fn test_sim_file_system_9() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1/2/3")),
            FileSystemOp::Open(PathBuf::from(""), true, true, true, false),
            FileSystemOp::Rename(PathBuf::from(""), PathBuf::from("1/2/3")),
            FileSystemOp::Write(12533627569028686350, "something".to_owned())
        ]));
    }

    #[test]
    fn test_sim_file_system_10() {
        assert!(check_sim_file_system(vec![
          FileSystemOp::CreateDirAll(PathBuf::from("1/2")),
           FileSystemOp::Open(PathBuf::from(""), true, true, true, false), 
          FileSystemOp::Write(18241168806576859958, "\"'삼\u{10}큺\"⁞\u{202b}!\u{86}\u{9d}\"|%\u{6dd}7%鼀\u{ffffe}\u{92}£/1k\u{8b}\u{7cfd8}\u{7}\u{8e}#․\u{82}6\u{93}\u{10ffff}©'f毽鎓‑".to_owned()), 
          FileSystemOp::Read(16832975082365959816, 784)
        ]));
    }

    #[test]
    fn test_sim_file_system_11() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("1/2/3")),
            FileSystemOp::Open(PathBuf::from("1/2"), true, true, false, false),
            FileSystemOp::Write(0, "1".to_owned()),
            FileSystemOp::Open(PathBuf::from("1/2"), true, true, false, false),
            FileSystemOp::Write(16905408381814530071, "2".to_owned()),
            FileSystemOp::Metadata(8885628981026197665)
        ]));
    }

    #[test]
    fn test_sim_file_system_12() {
        assert!(check_sim_file_system(vec![
            FileSystemOp::CreateDirAll(PathBuf::from("")),
            FileSystemOp::Open(PathBuf::from(""), true, true, true, true),
            FileSystemOp::Open(PathBuf::from(""), false, true, true, true),
            FileSystemOp::Open(PathBuf::from(""), true, true, true, false),
            FileSystemOp::Write(6717171345989498156, "\u{ab1a}+".to_owned()),
            FileSystemOp::Write(
                13967837215084670067,
                "0\u{9b}\u{2}x\u{95};‾T\u{84}‟4".to_owned() // "a".to_owned(),
            ),
            FileSystemOp::Open(PathBuf::from(""), true, true, false, false),
            FileSystemOp::Read(9189333954227666122, 996)
        ]));
    }

    #[test]
    fn test_sim_file_system_13() {
        assert!(check_sim_file_system(vec!
          [
            FileSystemOp::CreateDirAll(PathBuf::from("1/2/3")), 
            FileSystemOp::Open(PathBuf::from(""), true, true, false, true), 
            FileSystemOp::Open(PathBuf::from("1"), true, true, false, true), 
            FileSystemOp::Rename(PathBuf::from("1"), PathBuf::from("")), 
            FileSystemOp::Open(PathBuf::from(""), false, true, true, true), 
            FileSystemOp::Rename(PathBuf::from(""), PathBuf::from("1")), 
            FileSystemOp::Write(3533282638751806929, "⁄Bn⁔\"'A\u{7836f}@\u{8b}R\u{f2f1}\u{fffa}%W\u{6}5`s㼞3㘕\u{8}[%⁍헦,\t7\u{11}\t\u{206b}ੲ[\u{603}_1|\u{f261}¥\u{6}r@‿51\u{2007}\u{20d0}.+\u{b}\0\u{6dd}i¤I\u{202f}@.튃3\u{99}‑".to_owned())]
        ));
    }
}
