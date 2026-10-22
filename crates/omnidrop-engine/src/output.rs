//! OutputManager: stages results in temporary files next to their
//! destination and publishes them only after validation, without silently
//! replacing anything.

use crate::error::{EngineError, EngineResult, ErrorCode};
use crate::names::numbered_candidate;
use crate::operation::CollisionPolicy;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tempfile::{NamedTempFile, TempDir};

const TEMP_PREFIX: &str = ".omnidrop-";
const MAX_CANDIDATES: u32 = 10_000;

/// A result that exists only in a temporary location.
pub enum Staged {
    File(NamedTempFile),
    Dir(TempDir),
}

impl Staged {
    pub fn path(&self) -> &Path {
        match self {
            Staged::File(file) => file.path(),
            Staged::Dir(dir) => dir.path(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    pub path: PathBuf,
    /// The desired name was taken and a numbered one was used.
    pub renamed: bool,
}

pub struct OutputManager {
    policy: CollisionPolicy,
    /// Canonical paths of the job's inputs; never replaced.
    protected: HashSet<PathBuf>,
    /// Paths published by this job; never replaced by a later task.
    produced: Mutex<HashSet<PathBuf>>,
}

impl OutputManager {
    pub fn new<'a>(policy: CollisionPolicy, inputs: impl IntoIterator<Item = &'a Path>) -> Self {
        let protected = inputs
            .into_iter()
            .filter_map(|p| fs::canonicalize(p).ok())
            .collect();
        Self {
            policy,
            protected,
            produced: Mutex::new(HashSet::new()),
        }
    }

    pub fn ensure_dir(&self, dir: &Path) -> EngineResult<()> {
        fs::create_dir_all(dir).map_err(|e| EngineError::write(&e))
    }

    /// Creates an empty temporary file inside `dir` (same filesystem as the
    /// final location, so publishing is a rename).
    pub fn temp_file(&self, dir: &Path) -> EngineResult<NamedTempFile> {
        self.ensure_dir(dir)?;
        tempfile::Builder::new()
            .prefix(TEMP_PREFIX)
            .suffix(".tmp")
            .tempfile_in(dir)
            .map_err(|e| EngineError::write(&e))
    }

    pub fn temp_dir(&self, dir: &Path) -> EngineResult<TempDir> {
        self.ensure_dir(dir)?;
        tempfile::Builder::new()
            .prefix(TEMP_PREFIX)
            .tempdir_in(dir)
            .map_err(|e| EngineError::write(&e))
    }

    fn is_protected(&self, path: &Path) -> bool {
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.protected.contains(&canonical)
            || self.produced.lock().expect("lock").contains(&canonical)
    }

    fn remember(&self, path: &Path) {
        let canonical = fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
        self.produced.lock().expect("lock").insert(canonical);
    }

    /// Moves a validated result to its final place following the collision
    /// policy. Originals and earlier results of the same job are never
    /// replaced; with `Overwrite` they are kept by choosing a new name.
    pub fn publish(&self, staged: Staged, desired: &Path) -> EngineResult<Published> {
        let exists = desired.symlink_metadata().is_ok();
        let published = match self.policy {
            CollisionPolicy::Skip if exists => {
                return Err(EngineError::new(
                    ErrorCode::OutputExists,
                    "a file with this name already exists",
                ));
            }
            CollisionPolicy::Overwrite
                if exists && desired.is_file() && !self.is_protected(desired) =>
            {
                match staged {
                    Staged::File(file) => {
                        file.persist(desired)
                            .map_err(|e| EngineError::write(&e.error))?;
                        Published {
                            path: desired.to_path_buf(),
                            renamed: false,
                        }
                    }
                    staged @ Staged::Dir(_) => self.publish_numbered(staged, desired)?,
                }
            }
            CollisionPolicy::Skip => match self.try_place(staged, desired)? {
                Ok(path) => Published {
                    path,
                    renamed: false,
                },
                Err(_) => {
                    return Err(EngineError::new(
                        ErrorCode::OutputExists,
                        "a file with this name appeared while converting",
                    ));
                }
            },
            _ => self.publish_numbered(staged, desired)?,
        };
        self.remember(&published.path);
        Ok(published)
    }

    fn publish_numbered(&self, mut staged: Staged, desired: &Path) -> EngineResult<Published> {
        for n in 0..MAX_CANDIDATES {
            let candidate = if n == 0 {
                desired.to_path_buf()
            } else {
                numbered_candidate(desired, n)
            };
            match self.try_place(staged, &candidate)? {
                Ok(path) => {
                    return Ok(Published {
                        path,
                        renamed: n > 0,
                    });
                }
                Err(back) => staged = back,
            }
        }
        Err(EngineError::new(
            ErrorCode::DestinationNotWritable,
            "could not find a free file name",
        ))
    }

    /// Places `staged` at `target` only if nothing exists there. Returns the
    /// staged result back when the name is taken.
    fn try_place(&self, staged: Staged, target: &Path) -> EngineResult<Result<PathBuf, Staged>> {
        if target.symlink_metadata().is_ok() {
            return Ok(Err(staged));
        }
        match staged {
            Staged::File(file) => match file.persist_noclobber(target) {
                Ok(_) => Ok(Ok(target.to_path_buf())),
                Err(err) if err.error.kind() == io::ErrorKind::AlreadyExists => {
                    Ok(Err(Staged::File(err.file)))
                }
                Err(err) => Err(EngineError::write(&err.error)),
            },
            Staged::Dir(dir) => {
                let source = dir.keep();
                match fs::rename(&source, target) {
                    Ok(()) => Ok(Ok(target.to_path_buf())),
                    Err(err) => {
                        let _ = fs::remove_dir_all(&source);
                        if err.kind() == io::ErrorKind::AlreadyExists
                            || target.symlink_metadata().is_ok()
                        {
                            Err(EngineError::new(
                                ErrorCode::DestinationNotWritable,
                                "folder name was taken while publishing",
                            ))
                        } else {
                            Err(EngineError::write(&err))
                        }
                    }
                }
            }
        }
    }
}

/// Total size of a published file or folder.
pub fn size_on_disk(path: &Path) -> u64 {
    match fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => fs::read_dir(path)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|e| size_on_disk(&e.path()))
                    .sum()
            })
            .unwrap_or(0),
        Ok(meta) => meta.len(),
        Err(_) => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn staged_file(manager: &OutputManager, dir: &Path, content: &[u8]) -> Staged {
        let mut file = manager.temp_file(dir).unwrap();
        file.write_all(content).unwrap();
        Staged::File(file)
    }

    #[test]
    fn renames_on_collision() {
        let dir = tempfile::tempdir().unwrap();
        let desired = dir.path().join("photo.webp");
        fs::write(&desired, b"old").unwrap();
        let manager = OutputManager::new(CollisionPolicy::Rename, []);
        let published = manager
            .publish(staged_file(&manager, dir.path(), b"new"), &desired)
            .unwrap();
        assert_eq!(published.path, dir.path().join("photo (1).webp"));
        assert!(published.renamed);
        assert_eq!(fs::read(&desired).unwrap(), b"old");
        assert_eq!(fs::read(&published.path).unwrap(), b"new");
        // No temporary files are left behind.
        let leftovers: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with(TEMP_PREFIX))
            .collect();
        assert!(leftovers.is_empty());
    }

    #[test]
    fn overwrite_replaces_other_files_but_never_inputs() {
        let dir = tempfile::tempdir().unwrap();
        let original = dir.path().join("doc.pdf");
        fs::write(&original, b"original").unwrap();
        let other = dir.path().join("other.pdf");
        fs::write(&other, b"stale").unwrap();
        let manager = OutputManager::new(CollisionPolicy::Overwrite, [original.as_path()]);

        let published = manager
            .publish(staged_file(&manager, dir.path(), b"rotated"), &original)
            .unwrap();
        assert_eq!(published.path, dir.path().join("doc (1).pdf"));
        assert_eq!(fs::read(&original).unwrap(), b"original");

        let published = manager
            .publish(staged_file(&manager, dir.path(), b"fresh"), &other)
            .unwrap();
        assert_eq!(published.path, other);
        assert_eq!(fs::read(&other).unwrap(), b"fresh");

        // A second result of the same job does not replace the first.
        let published = manager
            .publish(staged_file(&manager, dir.path(), b"second"), &other)
            .unwrap();
        assert_eq!(published.path, dir.path().join("other (1).pdf"));
        assert_eq!(fs::read(&other).unwrap(), b"fresh");
    }

    #[test]
    fn skip_policy_reports_existing_output() {
        let dir = tempfile::tempdir().unwrap();
        let desired = dir.path().join("a.png");
        fs::write(&desired, b"keep").unwrap();
        let manager = OutputManager::new(CollisionPolicy::Skip, []);
        let err = manager
            .publish(staged_file(&manager, dir.path(), b"x"), &desired)
            .unwrap_err();
        assert_eq!(err.code, ErrorCode::OutputExists);
        assert_eq!(fs::read(&desired).unwrap(), b"keep");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
    }

    #[test]
    fn publishes_folders_with_numbering() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("Report")).unwrap();
        let manager = OutputManager::new(CollisionPolicy::Overwrite, []);
        let staging = manager.temp_dir(dir.path()).unwrap();
        fs::write(staging.path().join("p1.pdf"), b"x").unwrap();
        let published = manager
            .publish(Staged::Dir(staging), &dir.path().join("Report"))
            .unwrap();
        assert_eq!(published.path, dir.path().join("Report (1)"));
        assert!(published.path.join("p1.pdf").is_file());
        assert_eq!(size_on_disk(&published.path), 1);
    }

    #[test]
    fn dropped_staging_is_cleaned() {
        let dir = tempfile::tempdir().unwrap();
        let manager = OutputManager::new(CollisionPolicy::Rename, []);
        {
            let _staged = staged_file(&manager, dir.path(), b"partial");
            let _dir = manager.temp_dir(dir.path()).unwrap();
        }
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    }
}
