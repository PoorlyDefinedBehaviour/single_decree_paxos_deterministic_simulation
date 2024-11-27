use core::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::{collections::HashMap, path::PathBuf};

use crate::contracts::{self, FileSystem};

#[derive(Debug)]
pub struct SimFileSystem {
    pub cache: Rc<RefCell<HashMap<PathBuf, Rc<RefCell<FakeFile>>>>>,
    pub disk: Rc<RefCell<HashMap<PathBuf, FakeFile>>>,
}

impl Default for SimFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum FakeFileType {
    Dir,
    Data,
}

#[derive(Debug)]
pub struct FakeFile {
    pub typ: FakeFileType,
    pub path: PathBuf,
    pub data: Vec<u8>,
}

impl SimFileSystem {
    pub fn new() -> Self {
        SimFileSystem {
            cache: Rc::new(RefCell::new(HashMap::from([(
                PathBuf::from("tmp"),
                Rc::new(RefCell::new(FakeFile {
                    path: PathBuf::from("tmp"),
                    typ: FakeFileType::Dir,
                    data: Vec::new(),
                })),
            )]))),
            disk: Rc::new(RefCell::new(HashMap::new())),
        }
    }

    pub fn restart(&self) {
        let mut cache = self.cache.borrow_mut();

        cache.clear();

        for (path, file) in self.disk.borrow().iter() {
            cache.insert(
                path.to_owned(),
                Rc::new(RefCell::new(FakeFile {
                    typ: file.typ,
                    path: file.path.to_owned(),
                    data: file.data.clone(),
                })),
            );
        }
    }
}

fn is_dir_empty(cache: &HashMap<PathBuf, Rc<RefCell<FakeFile>>>, dir: &Path) -> bool {
    for path in cache.keys() {
        if let Some(parent) = path.parent() {
            if parent == dir {
                return false;
            }
        }
    }
    true
}

struct SimFile {
    cache: Rc<RefCell<HashMap<PathBuf, Rc<RefCell<FakeFile>>>>>,
    disk: Rc<RefCell<HashMap<PathBuf, FakeFile>>>,
    position: usize,
    open_options: contracts::OpenOptions,
    file: Rc<RefCell<FakeFile>>,
}

impl contracts::FileSystem for SimFileSystem {
    fn create_dir_all(&self, path: &std::path::Path) -> std::io::Result<()> {
        let mut cache = self.cache.borrow_mut();

        let mut current_path: PathBuf = PathBuf::new();

        let components: Vec<_> = path.components().collect();
        for (i, component) in components.iter().enumerate() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    current_path = current_path.join(std::path::Component::RootDir.as_os_str());
                }
                std::path::Component::ParentDir => {
                    continue;
                }
                std::path::Component::CurDir => {
                    current_path = current_path.join(".");
                }
                std::path::Component::Normal(dir) => {
                    current_path = current_path.join(dir);
                }
            }

            match cache.get(&current_path) {
                None => {
                    cache.insert(
                        current_path.clone(),
                        Rc::new(RefCell::new(FakeFile {
                            typ: FakeFileType::Dir,
                            path: path.to_owned(),
                            data: Vec::new(),
                        })),
                    );
                }
                Some(file) => {
                    let file = file.borrow();

                    if file.typ != FakeFileType::Dir {
                        // If it is the last component
                        if i == components.len() - 1 {
                            return Err(std::io::Error::new(std::io::ErrorKind::AlreadyExists, ""));
                        }

                        return Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, ""));
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
        match path.parent().map(PathBuf::from) {
            None => {
                // ignore.
            }
            Some(path) if path == PathBuf::from("") || path == PathBuf::from(".") => {
                // ignore.
            }
            Some(path) => {
                if !self.cache.borrow().contains_key(&path) {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "open: Cache doesn't contain parent path",
                    ));
                }
            }
        }

        let mut cache = self.cache.borrow_mut();

        if options.create && !cache.contains_key(path) {
            cache.insert(
                path.to_owned(),
                Rc::new(RefCell::new(FakeFile {
                    typ: FakeFileType::Data,
                    path: path.to_owned(),
                    data: Vec::new(),
                })),
            );
        }

        match cache.get_mut(path) {
            None => Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "open: Cache doesn't contain path",
            )),
            Some(file) => {
                if file.borrow().typ == FakeFileType::Dir
                    && (options.create || options.write || options.truncate)
                {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::IsADirectory,
                        "open: invalid open options",
                    ));
                }
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
        let mut cache = self.cache.borrow_mut();

        if from == to && cache.contains_key(from) {
            return Ok(());
        }

        if let Some(parent) = to.parent() {
            if !cache.contains_key(&PathBuf::from(parent)) {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::NotFound,
                    "No such file or directory",
                ));
            }
        }

        if !cache.contains_key(from) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No such file or directory",
            ));
        }

        if !is_dir_empty(&cache, to) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::DirectoryNotEmpty,
                "",
            ));
        }

        let file = cache.remove(from).unwrap();

        file.borrow_mut().path = to.to_owned();
        cache.insert(to.to_owned(), file);

        let keys: Vec<PathBuf> = cache
            .keys()
            .filter(|key| key.parent() == Some(from))
            .cloned()
            .collect();

        for key in keys {
            let new_key = key.to_str().unwrap().replace(
                from.as_os_str().to_str().unwrap(),
                to.as_os_str().to_str().unwrap(),
            );
            let file = cache.remove(&key).unwrap();
            file.borrow_mut().path = PathBuf::from(&new_key);
            cache.insert(PathBuf::from(&new_key), file);
        }

        Ok(())
    }
}

impl SimFile {
    fn new(
        cache: Rc<RefCell<HashMap<PathBuf, Rc<RefCell<FakeFile>>>>>,
        disk: Rc<RefCell<HashMap<PathBuf, FakeFile>>>,
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

        if file.typ == FakeFileType::Dir {
            return Err(std::io::Error::new(std::io::ErrorKind::IsADirectory, ""));
        }

        if self.position >= file.data.len() {
            return Ok(0);
        }

        let num_bytes_to_read =
            std::cmp::min(file.data.len().saturating_sub(self.position), buf.len());

        #[allow(clippy::manual_memcpy, clippy::needless_range_loop)]
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

    fn sync_all(&self) -> std::io::Result<()> {
        let file = self.file.borrow();

        if !self.cache.borrow().contains_key(&file.path) {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Uncategorized,
                "Bad file descriptor",
            ));
        }

        let mut disk = self.disk.borrow_mut();

        disk.insert(
            file.path.clone(),
            FakeFile {
                typ: file.typ,
                path: file.path.clone(),
                data: file.data.clone(),
            },
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use quickcheck::quickcheck;
    use std::io::{Read, Write};
    use uuid::Uuid;

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
            let path = if bool::arbitrary(g) {
                PathBuf::from("a")
            } else {
                PathBuf::from("b")
            };

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

                FileSystemOp::Open(path.join("filename"), create, write, read, truncate)
            } else if bool::arbitrary(g) {
                let new_path = if bool::arbitrary(g) {
                    PathBuf::from("a")
                } else {
                    PathBuf::from("b")
                };
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

                    assert_eq!(
                        std::fs::create_dir_all(&path).err().map(|err| err.kind()),
                        fs.create_dir_all(&path).err().map(|err| err.kind())
                    );
                }
                FileSystemOp::Open(path, create, write, read, truncate) => {
                    let path = dir.join(path);

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
                        assert_eq!(model_result.is_ok(), result.is_ok());
                    }
                }
                FileSystemOp::Rename(from, to) => {
                    let from_path = dir.join(from);
                    let to_path = dir.join(to);
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
                        let model_metadata = model_result.unwrap();
                        let metadata = result.unwrap();
                        if model_metadata.is_file() {
                            assert_eq!(model_metadata.len(), metadata.len());
                        }
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
        use FileSystemOp::*;

        check_sim_file_system(vec![
            CreateDirAll(PathBuf::from("b")),
            Open(PathBuf::from("b/filename"), true, true, false, false),
            Rename(PathBuf::from("b"), PathBuf::from("a")),
            Open(PathBuf::from("a/filename"), false, true, true, true),
        ]);
    }

    #[test]
    fn restart() {
        let fs = SimFileSystem::new();

        // Create a file.
        let mut file = fs
            .open(
                &PathBuf::from("./a.txt"),
                contracts::OpenOptions {
                    create: true,
                    write: true,
                    read: true,
                    truncate: false,
                },
            )
            .unwrap();

        // Write to the file.
        file.write_all("hello world".as_ref()).unwrap();

        // Pretend the computer was restarted before the file was synced to disk.
        fs.restart();

        // Reopen the file.
        let mut file = fs
            .open(
                &PathBuf::from("./a.txt"),
                contracts::OpenOptions {
                    create: true,
                    write: true,
                    read: true,
                    truncate: false,
                },
            )
            .unwrap();

        // Ensure the contents were lost because they were in the page cache.
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        assert!(s.is_empty());

        file.write_all("hello world".as_ref()).unwrap();

        // Sync file to disk.
        file.sync_all().unwrap();

        // Pretend the computer was restarted after the file was synced to disk.
        fs.restart();

        // Open the file.
        let mut file = fs
            .open(
                &PathBuf::from("./a.txt"),
                contracts::OpenOptions {
                    create: true,
                    write: true,
                    read: true,
                    truncate: false,
                },
            )
            .unwrap();

        // Ensure the data wasn't lost.
        let mut s = String::new();
        file.read_to_string(&mut s).unwrap();
        assert_eq!("hello world", s);
    }

    #[test]
    fn test_sync_all_dir() {
        let fs = SimFileSystem::new();
        fs.create_dir_all("./a/b/c".as_ref()).unwrap();
        let file = fs
            .open(
                "./a/b/c".as_ref(),
                contracts::OpenOptions {
                    create: false,
                    write: false,
                    read: true,
                    truncate: false,
                },
            )
            .unwrap();
        file.sync_all().unwrap();
        fs.restart();
        assert!(fs.cache.borrow().contains_key(&PathBuf::from("./a/b/c")));
    }
}
