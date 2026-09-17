//! Local immutable canonical snapshots. This reader uses only libgit2 object reads;
//! network features are disabled and it never invokes Git or a remote operation.
use crate::{
    error::{Error, Result},
    model::State,
    store::{Files, STATE_REF, Store},
};
use git2::{ErrorCode, Oid, Repository};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Reader {
    git_dir: PathBuf,
}
#[derive(Clone, Debug)]
pub struct ReaderSnapshot {
    pub oid: String,
    pub state: State,
}

fn repository_error(error: git2::Error) -> Error {
    let message = error.message();
    if message.contains("objectformat")
        || message.contains("object format")
        || message.contains("sha256")
    {
        Error::new(
            "OBJECT_FORMAT_UNSUPPORTED",
            "This reader supports SHA-1 Git repositories; SHA-256 repositories are not supported by this build",
        )
    } else if error.code() == ErrorCode::NotFound {
        Error::new("NOT_GIT_REPOSITORY", "Not inside a Git repository")
    } else {
        Error::new("GIT_ERROR", message)
    }
}
fn corrupt(error: git2::Error) -> Error {
    Error::new(
        "STATE_CORRUPT",
        format!("Cannot read local canonical state: {}", error.message()),
    )
}
impl Reader {
    /// Discover a repository without changing the process working directory.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let repo = Repository::discover(path).map_err(repository_error)?;
        Ok(Self {
            git_dir: repo.path().to_path_buf(),
        })
    }
    fn repository(&self) -> Result<Repository> {
        Repository::open(&self.git_dir).map_err(repository_error)
    }
    /// Observe the shared ref. A missing ref means aye has not been initialized.
    pub fn current_oid(&self) -> Result<Option<String>> {
        let repo = self.repository()?;
        match repo.find_reference(STATE_REF) {
            Ok(reference) => reference
                .resolve()
                .map_err(corrupt)?
                .target()
                .map(|oid| Some(oid.to_string()))
                .ok_or_else(|| {
                    Error::new("STATE_CORRUPT", "Task state reference has no object target")
                }),
            Err(error) if error.code() == ErrorCode::NotFound => Ok(None),
            Err(error) => Err(corrupt(error)),
        }
    }
    /// Read one full commit OID; later ref changes cannot mix this snapshot.
    pub fn load(&self, oid: &str) -> Result<ReaderSnapshot> {
        if oid.len() == 64 {
            return Err(Error::new(
                "OBJECT_FORMAT_UNSUPPORTED",
                "This reader supports SHA-1 Git object IDs",
            ));
        }
        if oid.len() != 40 || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::new(
                "STATE_CORRUPT",
                "Expected a full immutable commit OID",
            ));
        }
        let repo = self.repository()?;
        let id = Oid::from_str(oid).map_err(corrupt)?;
        let commit = repo.find_commit(id).map_err(corrupt)?;
        let root = commit.tree().map_err(corrupt)?;
        let mut files = Files::new();
        let project = root
            .get_name("project.json")
            .ok_or_else(|| Error::new("STATE_CORRUPT", "Missing project.json"))?;
        read_blob(&repo, &project, "project.json", &mut files)?;
        if let Some(tasks) = root.get_name("tasks") {
            if tasks.filemode_raw() != 0o040000 {
                return Err(Error::new("STATE_CORRUPT", "tasks must be a tree"));
            }
            let mut pending = vec![("tasks".to_string(), tasks.id())];
            while let Some((prefix, tree_oid)) = pending.pop() {
                let tree = repo.find_tree(tree_oid).map_err(corrupt)?;
                for entry in tree.iter() {
                    let name = entry.name().map_err(|_| {
                        Error::new("STATE_CORRUPT", "Non UTF-8 canonical state path")
                    })?;
                    let path = format!("{prefix}/{name}");
                    if entry.filemode_raw() == 0o040000 {
                        pending.push((path, entry.id()));
                    } else {
                        read_blob(&repo, &entry, &path, &mut files)?;
                    }
                }
            }
        }
        Ok(ReaderSnapshot {
            oid: id.to_string(),
            state: Store::parse(&files)?,
        })
    }
}
fn read_blob(
    repo: &Repository,
    entry: &git2::TreeEntry<'_>,
    path: &str,
    files: &mut Files,
) -> Result<()> {
    if entry.filemode_raw() != 0o100644 {
        return Err(Error::new(
            "STATE_CORRUPT",
            "Canonical state must contain ordinary JSON files",
        ));
    }
    let blob = repo.find_blob(entry.id()).map_err(corrupt)?;
    files.insert(path.into(), blob.content().to_vec());
    Ok(())
}
#[cfg(test)]
mod tests;
