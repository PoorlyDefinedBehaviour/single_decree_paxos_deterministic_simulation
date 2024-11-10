use crate::contracts::{self};
use anyhow::Result;
use std::{
    cell::RefCell,
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    rc::Rc,
};

pub struct FileStorage {
    fs: Rc<dyn contracts::FileSystem>,
    dir: PathBuf,
    file: RefCell<Box<dyn contracts::File>>,
    state: RefCell<Option<contracts::DurableState>>,
}

impl std::fmt::Debug for FileStorage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FileStorage")
            .field("dir", &self.dir)
            .finish()
    }
}

impl FileStorage {
    pub fn new(fs: Rc<dyn contracts::FileSystem>, dir: PathBuf) -> Result<Self> {
        // TODO: fsync dir.
        fs.create_dir_all(&dir)?;

        let path = dir.join("paxos.state");

        let mut file = create_or_open_file(fs.as_ref(), &path)?;

        let state = if file.metadata()?.is_empty() {
            None
        } else {
            let mut buffer = Vec::new();
            file.read_to_end(&mut buffer)?;
            serde_json::from_slice(&buffer)?
        };

        Ok(Self {
            fs,
            dir,
            file: RefCell::new(file),
            state: RefCell::new(state),
        })
    }
}

fn create_or_open_file(
    fs: &dyn contracts::FileSystem,
    path: &Path,
) -> std::io::Result<Box<dyn contracts::File>> {
    fs.open(
        path,
        contracts::OpenOptions {
            create: true,
            read: true,
            write: true,
            truncate: false,
        },
    )
}

fn create_or_truncate_file(
    fs: &dyn contracts::FileSystem,
    path: &Path,
) -> std::io::Result<Box<dyn contracts::File>> {
    fs.open(
        path,
        contracts::OpenOptions {
            create: true,
            read: true,
            write: true,
            truncate: true,
        },
    )
}

impl contracts::Storage for FileStorage {
    fn load(&self) -> contracts::DurableState {
        self.state
            .borrow()
            .clone()
            .unwrap_or(contracts::DurableState {
                min_proposal_number: 0,
                accepted_proposal_number: None,
                accepted_value: None,
            })
    }

    fn store(&self, state: &contracts::DurableState) -> std::io::Result<()> {
        let temp_file_path = self.dir.join("paxos.state.temp");
        let final_file_path = self.dir.join("paxos.state");
        let mut file = create_or_truncate_file(self.fs.as_ref(), &temp_file_path)?;
        file.write_all(serde_json::to_string(state).unwrap().as_ref())?;
        file.flush()?;
        self.fs.rename(&temp_file_path, &final_file_path)?;
        // TODO: need to fsync dir?
        *self.state.borrow_mut() = Some(state.to_owned());
        *self.file.borrow_mut() = file;
        Ok(())
    }
}

pub struct Fs {}

impl Default for Fs {
    fn default() -> Self {
        Self::new()
    }
}

impl Fs {
    pub fn new() -> Self {
        Self {}
    }
}

impl contracts::FileSystem for Fs {
    fn create_dir_all(&self, path: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(path)
    }

    fn open(
        &self,
        path: &std::path::Path,
        options: contracts::OpenOptions,
    ) -> std::io::Result<Box<dyn contracts::File>> {
        let file = OpenOptions::new()
            .create(options.create)
            .read(options.read)
            .write(options.write)
            .open(path)?;
        Ok(Box::new(file))
    }

    fn rename(&self, from: &std::path::Path, to: &std::path::Path) -> std::io::Result<()> {
        std::fs::rename(from, to)
    }
}

impl contracts::File for std::fs::File {
    fn metadata(&self) -> std::io::Result<contracts::Metadata> {
        self.metadata()
            .map(|m| contracts::Metadata { len: m.len() })
    }
}

#[cfg(test)]
mod tests {
    use contracts::Storage;
    use quickcheck::{quickcheck, Arbitrary};
    use uuid::Uuid;

    use crate::simulation::in_memory_storage::InMemoryStorage;

    use super::*;

    #[derive(Debug, Clone)]
    enum Op {
        New,
        Load,
        Store(u64, Option<u64>, Option<String>),
    }

    impl Arbitrary for Op {
        fn arbitrary(g: &mut quickcheck::Gen) -> Self {
            match u8::arbitrary(g) % 3 {
                0 => Op::New,
                1 => Op::Load,
                2 => Op::Store(
                    u64::arbitrary(g),
                    Option::<u64>::arbitrary(g),
                    Option::<String>::arbitrary(g),
                ),
                _ => unreachable!(),
            }
        }
    }

    quickcheck! {
      #[test]
      fn basic(ops: Vec<Op>) -> bool {
        let dir = std::env::temp_dir().join(Uuid::new_v4().to_string());
        let mut storage = FileStorage::new(Rc::new(Fs::new()), dir.clone()).unwrap();
        let model = InMemoryStorage::new();

        for op in ops {
          match op {
            Op::New => {
              storage = FileStorage::new(Rc::new(Fs::new()), dir.clone()).unwrap();
            }
              Op::Load => {
                assert_eq!(model.load(), storage.load());
              },
              Op::Store(min_proposal_number, accepted_proposal_number, accepted_value) => {
                let state = contracts::DurableState{
                  min_proposal_number,
                  accepted_proposal_number,
                  accepted_value
                };

                assert_eq!(model.store(&state).is_ok(), storage.store(&state).is_ok());
              },
          }
        }

        true
      }
    }
}
