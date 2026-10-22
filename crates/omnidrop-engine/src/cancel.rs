//! Cooperative cancellation shared between the UI thread and workers.

use crate::error::{EngineError, EngineResult};
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Cancellation seen by one task: the job-wide token or the task's own one.
#[derive(Debug, Clone, Default)]
pub struct TaskCancel {
    job: CancelToken,
    task: CancelToken,
}

impl TaskCancel {
    pub fn new(job: CancelToken, task: CancelToken) -> Self {
        Self { job, task }
    }

    pub fn is_cancelled(&self) -> bool {
        self.job.is_cancelled() || self.task.is_cancelled()
    }

    /// Returns `Err(Cancelled)` when cancellation was requested.
    pub fn check(&self) -> EngineResult<()> {
        if self.is_cancelled() {
            Err(EngineError::cancelled())
        } else {
            Ok(())
        }
    }
}

const CANCELLED_MARKER: &str = "omnidrop: cancelled";

/// Reader that fails with a recognizable error once cancellation is requested,
/// so long copies stop promptly.
pub struct CancellableReader<'a, R> {
    inner: R,
    cancel: &'a TaskCancel,
}

impl<'a, R: Read> CancellableReader<'a, R> {
    pub fn new(inner: R, cancel: &'a TaskCancel) -> Self {
        Self { inner, cancel }
    }
}

impl<R: Read> Read for CancellableReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.cancel.is_cancelled() {
            return Err(io::Error::other(CANCELLED_MARKER));
        }
        self.inner.read(buf)
    }
}

/// True when an I/O error was produced by [`CancellableReader`].
pub fn is_cancel_error(err: &io::Error) -> bool {
    err.kind() == io::ErrorKind::Other && err.to_string().contains(CANCELLED_MARKER)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_cancel_observes_job_and_task() {
        let job = CancelToken::new();
        let task = CancelToken::new();
        let combined = TaskCancel::new(job.clone(), task.clone());
        assert!(combined.check().is_ok());
        task.cancel();
        assert!(combined.check().unwrap_err().is_cancelled());

        let other = TaskCancel::new(job.clone(), CancelToken::new());
        assert!(!other.is_cancelled());
        job.cancel();
        assert!(other.is_cancelled());
    }

    #[test]
    fn cancellable_reader_stops() {
        let cancel = TaskCancel::default();
        let data = [1u8; 10];
        let mut reader = CancellableReader::new(&data[..], &cancel);
        let mut buf = [0u8; 4];
        assert_eq!(reader.read(&mut buf).unwrap(), 4);
        cancel.task.cancel();
        let err = reader.read(&mut buf).unwrap_err();
        assert!(is_cancel_error(&err));
    }
}
