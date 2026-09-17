use crate::error::{Error, Result};
use crate::model::{Project, State, Task};
use serde::Serialize;
use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

pub const STATE_REF: &str = "refs/agent-tasks/state";
pub type Files = BTreeMap<String, Vec<u8>>;

pub fn json_bytes(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

#[derive(Clone, Debug)]
pub struct Store {
    pub cwd: PathBuf,
    pub common: PathBuf,
    pub private: PathBuf,
}
#[derive(Clone)]
pub struct Snapshot {
    pub oid: String,
    pub state: State,
    pub files: Files,
    pub tasks_tree_oid: String,
}

impl Store {
    pub fn discover() -> Result<Self> {
        let cwd = std::env::current_dir()?;
        let mut store = Self {
            cwd,
            common: PathBuf::new(),
            private: PathBuf::new(),
        };
        store.private = PathBuf::from(store.git(&["rev-parse", "--absolute-git-dir"], None)?);
        store.common = PathBuf::from(store.git(
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            None,
        )?);
        Ok(store)
    }
    pub fn raw(&self, args: &[&str], input: Option<&[u8]>) -> Result<Output> {
        let mut child = Command::new("git")
            .args(args)
            .current_dir(&self.cwd)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_AUTHOR_NAME", "aye")
            .env("GIT_AUTHOR_EMAIL", "aye@localhost")
            .env("GIT_COMMITTER_NAME", "aye")
            .env("GIT_COMMITTER_EMAIL", "aye@localhost")
            .stdin(if input.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;
        let writer = input.map(|bytes| {
            let bytes = bytes.to_vec();
            let mut stdin = child.stdin.take().expect("piped stdin");
            std::thread::spawn(move || stdin.write_all(&bytes))
        });
        let output = child.wait_with_output()?;
        if let Some(writer) = writer {
            let result = writer
                .join()
                .map_err(|_| Error::new("GIT_ERROR", "Git input writer failed"))?;
            if output.status.success() {
                result?;
            }
        }
        Ok(output)
    }
    pub fn git(&self, args: &[&str], input: Option<&[u8]>) -> Result<String> {
        let output = self.raw(args, input)?;
        if !output.status.success() {
            return Err(Error::new(
                "GIT_ERROR",
                String::from_utf8_lossy(&output.stderr).trim(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
    pub fn tip(&self, reference: &str) -> Result<Option<String>> {
        let out = self.raw(&["rev-parse", "--verify", "--quiet", reference], None)?;
        if out.status.success() {
            Ok(Some(
                String::from_utf8_lossy(&out.stdout).trim().to_string(),
            ))
        } else if out.status.code() == Some(1) {
            Ok(None)
        } else {
            Err(Error::new(
                "GIT_ERROR",
                String::from_utf8_lossy(&out.stderr).trim(),
            ))
        }
    }
    pub fn head(&self) -> Result<Snapshot> {
        let oid = self
            .tip(STATE_REF)?
            .ok_or_else(|| Error::new("NOT_INITIALIZED", "Run aye init first"))?;
        self.load(&oid)
    }
    pub fn load(&self, oid: &str) -> Result<Snapshot> {
        let listing = self.raw(&["ls-tree", "-rz", "--full-tree", oid], None)?;
        if !listing.status.success() {
            return Err(Error::new("STATE_CORRUPT", "Cannot read state tree"));
        }
        let mut entries = Vec::new();
        for entry in listing.stdout.split(|b| *b == 0).filter(|e| !e.is_empty()) {
            let entry = std::str::from_utf8(entry)
                .map_err(|_| Error::new("STATE_CORRUPT", "Non UTF-8 state path"))?;
            let (header, path) = entry
                .split_once('\t')
                .ok_or_else(|| Error::new("STATE_CORRUPT", "Invalid tree entry"))?;
            let parts: Vec<_> = header.split_whitespace().collect();
            if parts.len() != 3 {
                return Err(Error::new("STATE_CORRUPT", "Invalid tree entry"));
            }
            if parts[0] != "100644" || parts[1] != "blob" {
                if path == "project.json" || path == "tasks" || path.starts_with("tasks/") {
                    return Err(Error::new(
                        "STATE_CORRUPT",
                        "Canonical state must contain ordinary JSON files",
                    ));
                }
                // Disposable projections with invalid modes are treated as
                // missing. Never follow a link or parse them as canonical data.
                continue;
            }
            if path == "tasks" {
                return Err(Error::new("STATE_CORRUPT", "tasks must be a tree"));
            }
            entries.push((path.to_string(), parts[2].to_string()));
        }
        let request: String = entries.iter().map(|(_, oid)| format!("{oid}\n")).collect();
        let output = self.raw(&["cat-file", "--batch"], Some(request.as_bytes()))?;
        if !output.status.success() {
            return Err(Error::new("STATE_CORRUPT", "Cannot read state objects"));
        }
        let mut cursor = output.stdout.as_slice();
        let mut files = Files::new();
        for (path, _) in entries {
            let newline = cursor
                .iter()
                .position(|b| *b == b'\n')
                .ok_or_else(|| Error::new("STATE_CORRUPT", "Truncated object header"))?;
            let header = String::from_utf8_lossy(&cursor[..newline]);
            let size = header
                .split_whitespace()
                .nth(2)
                .and_then(|s| s.parse::<usize>().ok())
                .ok_or_else(|| Error::new("STATE_CORRUPT", "Invalid object header"))?;
            cursor = &cursor[newline + 1..];
            if cursor.len() <= size {
                return Err(Error::new("STATE_CORRUPT", "Truncated object"));
            }
            files.insert(path, cursor[..size].to_vec());
            cursor = &cursor[size + 1..];
        }
        let state = Self::parse(&files)?;
        let tasks_tree_oid = match self.tip(&format!("{oid}:tasks"))? {
            Some(oid) => oid,
            None => self.git(&["hash-object", "-t", "tree", "--stdin"], Some(b""))?,
        };
        Ok(Snapshot {
            oid: oid.to_string(),
            state,
            files,
            tasks_tree_oid,
        })
    }
    pub fn parse(files: &Files) -> Result<State> {
        let project: Project = serde_json::from_slice(
            files
                .get("project.json")
                .ok_or_else(|| Error::new("STATE_CORRUPT", "Missing project.json"))?,
        )?;
        if project.format_version > 1 {
            return Err(Error::new(
                "FORMAT_VERSION_UNSUPPORTED",
                "This tool supports format version 1",
            ));
        }
        let mut tasks = BTreeMap::new();
        for (path, bytes) in files {
            if path.starts_with("tasks/") {
                let task: Task = serde_json::from_slice(bytes)?;
                // Validate ID before slicing it for the canonical path.
                if task.id.len() != 22
                    || !task.id.starts_with("t-")
                    || !task.id[2..]
                        .bytes()
                        .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
                {
                    return Err(Error::new("STATE_CORRUPT", "Invalid task ID"));
                }
                if task.path() != *path {
                    return Err(Error::new(
                        "STATE_CORRUPT",
                        format!("Task ID/path mismatch: {path}"),
                    ));
                }
                tasks.insert(task.id.clone(), task);
            }
        }
        let state = State { project, tasks };
        state.validate()?;
        Ok(state)
    }
    pub fn canonical(state: &State, original: Option<&Snapshot>) -> Result<Files> {
        let mut files = Files::new();
        let project = if let Some(old) = original {
            if serde_json::to_value(&state.project)? == serde_json::to_value(&old.state.project)? {
                old.files["project.json"].clone()
            } else {
                json_bytes(&state.project)?
            }
        } else {
            json_bytes(&state.project)?
        };
        files.insert("project.json".into(), project);
        for task in state.tasks.values() {
            let path = task.path();
            let bytes = if let Some(old) = original {
                if old
                    .state
                    .tasks
                    .get(&task.id)
                    .map(serde_json::to_value)
                    .transpose()?
                    == Some(serde_json::to_value(task)?)
                {
                    old.files[&path].clone()
                } else {
                    json_bytes(task)?
                }
            } else {
                json_bytes(task)?
            };
            files.insert(path, bytes);
        }
        Ok(files)
    }
    pub fn write_tree(&self, files: &Files) -> Result<String> {
        // Import only blobs in one process: no commit/reset/tag commands, so
        // fast-import cannot move refs or touch any source index/worktree.
        let mut input = Vec::new();
        for (index, bytes) in files.values().enumerate() {
            input.extend_from_slice(
                format!("blob\nmark :{}\ndata {}\n", index + 1, bytes.len()).as_bytes(),
            );
            input.extend_from_slice(bytes);
            input.extend_from_slice(format!("\nget-mark :{}\n", index + 1).as_bytes());
        }
        input.extend_from_slice(b"done\n");
        let imported = self.git(&["fast-import", "--quiet", "--done"], Some(&input))?;
        let oids: Vec<_> = imported.lines().collect();
        if oids.len() != files.len() {
            return Err(Error::new(
                "GIT_ERROR",
                "Blob import returned an unexpected object count",
            ));
        }
        // Build every shard at the same depth in one mktree batch, then
        // reference those OIDs from their parents. Process count follows depth,
        // rather than the number of tasks or shards.
        let mut directories: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        directories.insert(String::new(), Vec::new());
        directories.insert("tasks".into(), Vec::new());
        for ((path, _), oid) in files.iter().zip(oids) {
            if path.split('/').any(|name| {
                name.is_empty() || name == "." || name == ".." || name.contains(['\0', '\n', '\t'])
            }) {
                return Err(Error::new("STATE_CORRUPT", "Invalid state path"));
            }
            let (directory, name) = path.rsplit_once('/').unwrap_or(("", path));
            directories
                .entry(directory.into())
                .or_default()
                .extend_from_slice(format!("100644 blob {oid}\t{name}\0").as_bytes());
            let mut parent = directory;
            while let Some((ancestor, _)) = parent.rsplit_once('/') {
                directories.entry(ancestor.into()).or_default();
                parent = ancestor;
            }
        }
        let depth = |path: &str| {
            if path.is_empty() {
                0
            } else {
                path.split('/').count()
            }
        };
        let max_depth = directories.keys().map(|p| depth(p)).max().unwrap_or(0);
        for level in (0..=max_depth).rev() {
            let paths: Vec<_> = directories
                .keys()
                .filter(|p| depth(p) == level)
                .cloned()
                .collect();
            let mut input = Vec::new();
            for path in &paths {
                input.extend_from_slice(&directories[path]);
                input.push(0);
            }
            let output = self.git(&["mktree", "--batch", "-z"], Some(&input))?;
            let oids: Vec<_> = output.lines().collect();
            if paths.len() != oids.len() {
                return Err(Error::new(
                    "GIT_ERROR",
                    "Tree batch returned an unexpected object count",
                ));
            }
            for (path, oid) in paths.iter().zip(oids) {
                if path.is_empty() {
                    return Ok(oid.into());
                }
                let (parent, name) = path.rsplit_once('/').unwrap_or(("", path));
                directories
                    .entry(parent.into())
                    .or_default()
                    .extend_from_slice(format!("040000 tree {oid}\t{name}\0").as_bytes());
            }
        }
        Err(Error::new("GIT_ERROR", "No root tree generated"))
    }

    pub fn commit(&self, files: &Files, parents: &[&str], message: &str) -> Result<String> {
        let tree = self.write_tree(files)?;
        let mut args = vec!["-c", "commit.gpgsign=false", "commit-tree", &tree];
        for parent in parents {
            args.extend(["-p", parent]);
        }
        self.git(&args, Some(message.as_bytes()))
    }
    pub fn metadata(&self) -> PathBuf {
        self.common.join("agent-tasks")
    }
    pub fn conflict_path(&self) -> PathBuf {
        self.metadata().join("conflicts/pending.json")
    }
    pub fn lock(&self) -> Result<File> {
        fs::create_dir_all(self.metadata())?;
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.metadata().join("publication.lock"))?;
        file.lock()?;
        Ok(file)
    }
    pub fn ensure_writable(&self) -> Result<()> {
        if self.conflict_path().exists() {
            Err(Error::new(
                "SYNC_CONFLICT",
                "Resolve or abort the pending sync conflict first",
            ))
        } else {
            Ok(())
        }
    }
    pub fn cas_unlocked(&self, old: Option<&str>, new: &str) -> Result<bool> {
        let output = self.raw(
            &[
                "update-ref",
                "--no-deref",
                "--create-reflog",
                "-m",
                "aye state update",
                STATE_REF,
                new,
                old.unwrap_or(""),
            ],
            None,
        )?;
        if output.status.success() {
            return Ok(true);
        }
        if self.tip(STATE_REF)?.as_deref() != old {
            return Ok(false);
        }
        Err(Error::new(
            "GIT_ERROR",
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }
    pub fn cas(&self, old: Option<&str>, new: &str) -> Result<bool> {
        let _lock = self.lock()?;
        self.ensure_writable()?;
        self.cas_unlocked(old, new)
    }
    pub fn project_files(&self, state: &State, mut canonical: Files) -> Result<Files> {
        canonical.retain(|path, _| path == "project.json" || path.starts_with("tasks/"));
        state.validate()?;
        let tree = self.write_tree(&canonical)?;
        let tasks_oid = self.git(&["rev-parse", &format!("{tree}:tasks")], None)?;
        canonical.extend(crate::projection::build(state, &tasks_oid)?);
        Ok(canonical)
    }
    pub fn rebuild(&self) -> Result<Snapshot> {
        for _ in 0..12 {
            self.ensure_writable()?;
            let old = self.head()?;
            let files = self.project_files(&old.state, old.files.clone())?;
            if files == old.files {
                return Ok(old);
            }
            let oid = self.commit(&files, &[&old.oid], "aye rebuild projections")?;
            if self.cas(Some(&old.oid), &oid)? {
                return self.load(&oid);
            }
        }
        Err(Error::new(
            "LOCAL_CONCURRENCY_RETRY_EXHAUSTED",
            "State changed during rebuild",
        ))
    }
    pub fn initialize_offline(&self) -> Result<Snapshot> {
        if let Some(oid) = self.tip(STATE_REF)? {
            return self.load(&oid);
        }
        let state = State::empty();
        let files = self.project_files(&state, Self::canonical(&state, None)?)?;
        let oid = self.commit(&files, &[], "aye init")?;
        self.cas(None, &oid)?;
        self.head()
    }
    pub fn mutate(
        &self,
        mut operation: impl FnMut(&mut State) -> Result<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        for _ in 0..12 {
            self.ensure_writable()?;
            let old = self.head()?;
            let mut state = old.state.clone();
            let value = operation(&mut state)?;
            state.validate()?;
            let files = self.project_files(&state, Self::canonical(&state, Some(&old))?)?;
            if files == old.files {
                return Ok(value);
            }
            let oid = self.commit(&files, &[&old.oid], "aye task mutation")?;
            if self.cas(Some(&old.oid), &oid)? {
                return Ok(value);
            }
        }
        Err(Error::new(
            "LOCAL_CONCURRENCY_RETRY_EXHAUSTED",
            "Task state changed repeatedly; retry command",
        ))
    }
    pub fn actor(&self, explicit: Option<&str>) -> Result<Option<String>> {
        let actor = explicit
            .map(str::to_string)
            .or_else(|| std::env::var("AYE_ACTOR").ok())
            .or_else(|| fs::read_to_string(self.private.join("agent-tasks/actor")).ok());
        Ok(actor
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty()))
    }
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::new("IO_ERROR", "Missing parent path"))?;
    fs::create_dir_all(parent)?;
    let staging = parent.join(format!(".write-{}", uuid::Uuid::new_v4()));
    fs::write(&staging, bytes)?;
    fs::rename(&staging, path)?;
    Ok(())
}
